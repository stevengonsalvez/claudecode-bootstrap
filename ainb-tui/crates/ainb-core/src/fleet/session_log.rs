// ABOUTME: The `log` tab's notification history, read off the render thread on
// one long-lived read-only handle.
//
// It used to be read INSIDE `terminal.draw`: every repaint of the `log` tab
// called `Store::open` (read-write, `PRAGMA journal_mode=WAL`, the full
// `CREATE ... IF NOT EXISTS` batch), pulled up to 4000 rows, and closed the
// connection again. Three separate costs, all on the UI thread:
//
// 1. the open itself, which on a store the daemon has been writing to for
//    months is not free;
// 2. the schema migration, which is the TUI applying DDL to a database the
//    daemon owns;
// 3. the CLOSE, which on the last connection CHECKPOINTS the WAL. Measured
//    against a byte-copy of a real store (546 MB file, 9 927 rows, 73.5 MB
//    WAL): 948 ms for the frame that inherited the fat WAL, 37-631 ms per
//    frame after that.
//
// The render loop repaints on every keystroke and every 250 ms app tick, so
// that is a pane which answers a `Tab` press a second later. This module is
// where that read went: one worker thread, one read-only connection held open,
// a refresh cadence instead of a frame cadence, and a render path that only
// ever reads a `Vec` someone else filled in.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::components::session_tabs::LogRow;

/// How often the worker re-reads while the pane stays on one session.
///
/// A notification history is not a live stream — a row that appears half a
/// second late is indistinguishable from instant. A CHANGE of session does not
/// wait for this: the render path signals the condvar, so switching rows
/// repaints as fast as the query returns.
const REFRESH: Duration = Duration::from_millis(750);

/// How long the worker sleeps before retrying after a failed read.
///
/// Longer than [`REFRESH`] on purpose: the usual failure is "the store does not
/// exist on this machine", which will still be true 750 ms from now, and
/// retrying it at the refresh cadence is a syscall per second forever.
const RETRY: Duration = Duration::from_secs(5);

/// Which session's history is wanted.
///
/// The worktree path and the agent's hook name, which is the same identity the
/// attention producer files notifyd rows under — a session is not identified in
/// that store by ainb's own UUID, which the hook never sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogKey {
    /// The session's worktree path, trailing slash trimmed.
    pub cwd: String,
    /// The agent's hook name, or `None` to accept every agent in that cwd.
    pub agent: Option<String>,
}

impl LogKey {
    /// The key for a session at `cwd` running `agent`.
    #[must_use]
    pub fn new(cwd: &str, agent: Option<&str>) -> Self {
        Self {
            cwd: cwd.trim_end_matches('/').to_string(),
            agent: agent.map(ToString::to_string),
        }
    }
}

/// What the worker has published, and what it should read next.
#[derive(Debug, Default)]
struct Published {
    /// The key the render path last asked for.
    want: Option<LogKey>,
    /// The key `rows` actually belongs to, so a stale answer is never rendered
    /// under a session it is not about.
    have: Option<LogKey>,
    /// The rows for `have`.
    rows: Vec<LogRow>,
    /// Why the last read failed, for the one line the pane shows instead of
    /// pretending the session has no history.
    error: Option<String>,
    /// Whether the render path has asked for rows since the last read.
    ///
    /// This is what stops the worker outliving the pane. Set by every frame
    /// that wants the log, cleared by every read the worker completes: once the
    /// operator leaves the tab nothing sets it again, the worker parks on the
    /// condvar, and a session nobody is looking at stops costing a query every
    /// 750 ms for the rest of the process's life.
    asked: bool,
}

/// The cell the render loop reads and the worker writes.
#[derive(Debug, Default)]
pub struct Shared {
    published: Mutex<Published>,
    /// Signalled when `want` changes, so a session switch does not wait out
    /// [`REFRESH`].
    wake: Condvar,
}

/// What the `log` pane has to paint right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Log {
    /// The history for the requested session.
    Rows(Vec<LogRow>),
    /// The worker has not answered for THIS session yet.
    Reading,
    /// The read failed, with the reason.
    Failed(String),
}

impl Shared {
    fn guard(&self) -> std::sync::MutexGuard<'_, Published> {
        self.published.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Ask for `key`, and return whatever is available for it right now.
    ///
    /// One call, not a request and a separate read: the render path wants the
    /// rows for the session under the cursor, and splitting that into two
    /// locked operations is how a caller ends up rendering the previous
    /// session's history under the current session's name.
    #[must_use]
    pub fn read(&self, key: &LogKey) -> Log {
        let mut published = self.guard();
        if published.want.as_ref() != Some(key) {
            published.want = Some(key.clone());
        }
        if !published.asked {
            published.asked = true;
            // Woken while this lock is still held; the worker simply blocks on
            // the mutex until the guard drops at the end of the function. Only
            // on the FALSE->TRUE edge, because that is the only transition a
            // parked worker is waiting for — notifying on every frame would be
            // eighty wakeups a second telling it something it already knows.
            self.wake.notify_one();
        }
        if published.have.as_ref() == Some(key) {
            return Log::Rows(published.rows.clone());
        }
        match &published.error {
            Some(reason) => Log::Failed(reason.clone()),
            None => Log::Reading,
        }
    }
}

/// Start the worker, or return immediately when one is already running.
///
/// Idempotent by an atomic flag, and released on every exit path including an
/// unwind, so a worker that dies is replaced on the next frame rather than
/// leaving the pane permanently stuck on "reading". Same shape, and for the
/// same reason, as [`crate::fleet::attention_poll::spawn`].
pub fn spawn(shared: &Arc<Shared>, running: &Arc<AtomicBool>) {
    if running.swap(true, Ordering::AcqRel) {
        return;
    }
    let shared = Arc::clone(shared);
    let worker_flag = Arc::clone(running);
    let spawn_err_flag = Arc::clone(running);
    let spawned = std::thread::Builder::new().name("ainb-session-log".into()).spawn(move || {
        struct Guard(Arc<AtomicBool>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _guard = Guard(worker_flag);
        worker(&shared);
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "session log worker thread spawn failed");
        spawn_err_flag.store(false, Ordering::Release);
    }
}

/// How many rows the pane shows.
///
/// A history an operator scrolls, not a stream: 200 lines is more than anyone
/// reads in one sitting and bounds what the query has to carry.
const LIMIT: u32 = 200;

/// The worker loop: read what is wanted, publish it, wait for a change or the
/// refresh cadence.
fn worker(shared: &Shared) {
    // Opened once and held. The per-call open is the cost this module exists to
    // remove, and a read-only handle additionally cannot migrate the daemon's
    // schema or checkpoint its WAL.
    let mut store: Option<ainb_plugin_notifyd::Store> = None;
    loop {
        let (want, asked) = {
            let published = shared.guard();
            (published.want.clone(), published.asked)
        };
        let mut backoff = REFRESH;
        if let (true, Some(key)) = (asked, want) {
            match read_once(&mut store, &key) {
                Ok(rows) => {
                    let mut published = shared.guard();
                    published.have = Some(key);
                    published.rows = rows;
                    published.error = None;
                    published.asked = false;
                }
                Err(reason) => {
                    // Drop the handle: the usual causes (the file was replaced,
                    // the daemon rebuilt it) are not fixed by reusing it.
                    store = None;
                    let mut published = shared.guard();
                    published.error = Some(reason);
                    published.asked = false;
                    backoff = RETRY;
                }
            }
        }
        let published = shared.guard();
        // A request that arrived while the read was in flight is served now
        // rather than after a full interval — without this, the pane would show
        // the previous session for up to `REFRESH` after the cursor moved.
        if published.asked && published.want != published.have && published.error.is_none() {
            continue;
        }
        if published.asked {
            // The pane is open and satisfied: sleep the refresh interval, then
            // read again.
            let _unused = shared.wake.wait_timeout(published, backoff);
        } else {
            // Nobody has asked since the last read, so there is nothing to
            // refresh FOR. Park until a frame asks again.
            //
            // Checked while holding the guard, and `wait` releases it
            // atomically, so a `read` that lands between the check and the wait
            // cannot have its notify lost — it would still be blocked on this
            // mutex.
            let _unused = shared.wake.wait(published);
        }
    }
}

/// One read, against a handle opened on demand.
fn read_once(
    store: &mut Option<ainb_plugin_notifyd::Store>,
    key: &LogKey,
) -> Result<Vec<LogRow>, String> {
    if store.is_none() {
        let paths = ainb_plugin_notifyd::Paths::from_home().map_err(|error| error.to_string())?;
        if !paths.db.exists() {
            return Err(format!(
                "no notification store at {} yet",
                paths.db.display()
            ));
        }
        // READ-ONLY. The TUI is a reader of a database the daemon owns: it must
        // not create it, must not apply DDL to it, and must not checkpoint its
        // WAL — the last of which is what made a single frame cost 948 ms.
        *store = Some(
            ainb_plugin_notifyd::Store::open_readonly(&paths.db)
                .map_err(|error| error.to_string())?,
        );
    }
    let store = store.as_ref().expect("opened above");
    // Filtered in SQL, not here. Reading the newest 4000 rows fleet-wide and
    // keeping the handful for one cwd is what made this expensive enough to
    // matter — see `Store::recent_for_cwd`.
    let records = store
        .recent_for_cwd(&key.cwd, key.agent.as_deref(), 0, LIMIT)
        .map_err(|error| error.to_string())?;
    Ok(crate::components::session_tabs::log_rows(
        &records,
        &key.cwd,
        key.agent.as_deref(),
        LIMIT as usize,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(ts: i64) -> LogRow {
        LogRow {
            ts,
            event: "Notification".to_string(),
            detail: "hello".to_string(),
        }
    }

    #[test]
    fn a_key_normalises_its_trailing_slash() {
        assert_eq!(
            LogKey::new("/tmp/work/", Some("claude")),
            LogKey::new("/tmp/work", Some("claude"))
        );
    }

    #[test]
    fn an_unanswered_key_reads_as_reading_not_as_empty() {
        let shared = Shared::default();
        // The distinction the pane needs: "no rows yet" is not "this session
        // has no history", and rendering the empty state for the first is how
        // an operator concludes their log is gone.
        assert_eq!(shared.read(&LogKey::new("/tmp/a", None)), Log::Reading);
    }

    #[test]
    fn rows_are_only_returned_for_the_key_they_were_read_for() {
        let shared = Shared::default();
        let a = LogKey::new("/tmp/a", None);
        let b = LogKey::new("/tmp/b", None);
        {
            let mut published = shared.guard();
            published.have = Some(a.clone());
            published.rows = vec![row(1)];
        }
        assert_eq!(shared.read(&a), Log::Rows(vec![row(1)]));
        // `b` must NOT inherit `a`'s history just because it is what the worker
        // last published.
        assert_eq!(shared.read(&b), Log::Reading);
    }

    #[test]
    fn reading_records_what_the_worker_should_fetch_next() {
        let shared = Shared::default();
        let key = LogKey::new("/tmp/a", Some("claude"));
        let _unused = shared.read(&key);
        assert_eq!(shared.guard().want, Some(key));
    }

    /// The worker must not outlive the pane. Every frame that wants rows raises
    /// `asked`; the worker lowers it after each read and parks when it is down,
    /// so leaving the tab stops the query rather than leaving it running for
    /// the rest of the process's life.
    #[test]
    fn a_pane_nobody_is_looking_at_stops_asking() {
        let shared = Shared::default();
        let key = LogKey::new("/tmp/a", None);
        let _unused = shared.read(&key);
        assert!(
            shared.guard().asked,
            "a frame that wants rows asks for them"
        );

        // The worker completing a read.
        {
            let mut published = shared.guard();
            published.have = Some(key.clone());
            published.rows = vec![row(1)];
            published.asked = false;
        }
        // No further frames: nothing raises it again, so the worker parks.
        assert!(
            !shared.guard().asked,
            "a closed pane must leave nothing for the worker to refresh"
        );

        // Re-opening the pane raises it again.
        assert_eq!(shared.read(&key), Log::Rows(vec![row(1)]));
        assert!(
            shared.guard().asked,
            "and a frame that comes back asks again"
        );
    }

    #[test]
    fn a_failed_read_is_reported_rather_than_rendered_as_no_history() {
        let shared = Shared::default();
        {
            let mut published = shared.guard();
            published.error = Some("no notification store yet".to_string());
        }
        assert_eq!(
            shared.read(&LogKey::new("/tmp/a", None)),
            Log::Failed("no notification store yet".to_string())
        );
    }
}
