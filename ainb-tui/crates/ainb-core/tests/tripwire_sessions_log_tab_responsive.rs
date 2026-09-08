//! Tripwire: the `log` tab stops reading SQLite inside `terminal.draw`.
//!
//! The report was "in the logs folder the app gets hung". It was not a hang —
//! it was a store read on the RENDER THREAD. `render_session_tab`'s `Log` arm
//! called `session_tabs::read_log`, which per repaint opened the notifyd store
//! read-write, re-ran the whole `CREATE ... IF NOT EXISTS` batch, pulled the
//! newest 4000 rows with their payloads, and closed the connection again.
//!
//! Measured with `Store::open` + `recent_since(0, 4000)` against a 138 MB store
//! (the shape this test seeds): 2 494 ms for the first call and 106-250 ms for
//! every one after. The loop repaints once per keystroke, so every key an
//! operator pressed on this tab paid for one of those reads. With the TUI's own
//! `AINB_PERF_TRACE` on the fixture below, key-to-render went from p50 53 ms /
//! max 467 ms to p50 6.6 ms / max 45 ms once the read moved off the thread.
//!
//! Two things have to be true on a real screen, and neither is provable from a
//! unit test:
//!
//! 1. the pane still shows this session's history — the fix must not have
//!    bought responsiveness by rendering nothing;
//! 2. with the pane OPEN on a large store, a burst of keystrokes is answered
//!    promptly. The burst is the amplifier: the loop paints once per key, so N
//!    keys cost N reads under the old code and none under the new one.
//!
//! Deliberately store-heavy and not daemon-backed: the local notifications
//! store is the thing being read, and the hangar has no part in it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use ainb_plugin_notifyd::{Envelope, Paths, Store};
use serde_json::json;

/// Rows the store carries. The old read took the newest `200 * 20`, so the
/// seed has to clear 4000 for the query to be doing its full work.
const FILLER_ROWS: usize = 4_100;

/// Payload bytes per filler row. 32 KB × 4000 rows is ~130 MB of payload text
/// that the old path materialised on EVERY frame to render sixty lines.
const PAYLOAD_BYTES: usize = 32 * 1024;

/// Rows belonging to the session under test, so the pane has real history.
const MINE_ROWS: usize = 40;

/// The message the pane must render, distinct from the filler.
const MINE_MESSAGE: &str = "earlier turn ended here";

/// The raw hook event beside it. Only the `log` pane prints this.
const MINE_EVENT: &str = "Notification:idle_prompt";

/// Inert keys in the responsiveness burst.
///
/// `F5` is bound to nothing on this screen, but the loop repaints on ANY input
/// event — so each one costs exactly one repaint of the log pane and changes
/// nothing else. That is the multiplier on the per-frame cost.
///
/// A burst of `?` would not work: it toggles the help overlay, so the overlay
/// appears after the FIRST key and the poll would stop before the burst had
/// drained. The single `?` below goes at the END, where the overlay it opens
/// can only be reached through all `BURST_KEYS` repaints ahead of it.
const BURST_KEYS: usize = 200;

/// How long the burst may take to drain.
///
/// Measured on this fixture, same machine, same run: 16.6 s with the read on
/// the render thread, 0.9 s with it on the worker. Five seconds sits between
/// the two with 5x headroom below and 3x above, rather than splitting the
/// difference and flaking on whichever side the machine is slow on that day.
const BURST_DEADLINE: Duration = Duration::from_secs(5);

fn ainb_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ainb"))
}

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

struct ExactTmuxSession {
    name: String,
}

impl ExactTmuxSession {
    fn create(name: String, command: &[&str]) -> Self {
        let mut args: Vec<String> =
            ["new-session", "-d", "-s"].iter().map(|a| (*a).to_string()).collect();
        args.push(name.clone());
        args.extend(["-x", "200", "-y", "50"].iter().map(|a| (*a).to_string()));
        args.extend(command.iter().map(|part| (*part).to_string()));
        let status = Command::new("tmux").args(&args).status().expect("tmux new-session");
        assert!(status.success(), "tmux new-session {name} failed");
        Self { name }
    }
}

impl Drop for ExactTmuxSession {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &format!("={}", self.name)])
            .status();
    }
}

fn capture_pane(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-t", session, "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn send_key(session: &str, key: &str) {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", session, key])
        .status()
        .expect("tmux send-keys");
    assert!(status.success(), "tmux send-keys {key:?} failed");
}

fn poll<F>(session: &str, deadline: Instant, mut ok: F) -> Option<String>
where
    F: FnMut(&str) -> bool,
{
    while Instant::now() < deadline {
        let cap = capture_pane(session);
        if ok(&cap) {
            return Some(cap);
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

fn poll_until<F, G>(
    session: &str,
    key: &str,
    deadline: Instant,
    mut arrived: G,
    mut ok: F,
) -> Option<String>
where
    F: FnMut(&str) -> bool,
    G: FnMut(&str) -> bool,
{
    let mut on_screen = false;
    while Instant::now() < deadline {
        let cap = capture_pane(session);
        if ok(&cap) {
            return Some(cap);
        }
        on_screen = on_screen || arrived(&cap);
        if !on_screen {
            send_key(session, key);
        }
        thread::sleep(Duration::from_millis(400));
    }
    None
}

fn press_until<F>(
    session: &str,
    key: &str,
    attempts: usize,
    mut ok: F,
) -> Result<String, Vec<String>>
where
    F: FnMut(&str) -> bool,
{
    let mut seen = Vec::new();
    for _ in 0..attempts {
        // Generous: on a big store the OLD code needed most of a second per
        // frame, and a test that gave up sooner would fail as "never reached
        // the pane" rather than reporting the slowness it exists to catch.
        for _ in 0..12 {
            let cap = capture_pane(session);
            if ok(&cap) {
                return Ok(cap);
            }
            if seen.last() != Some(&cap) {
                seen.push(cap);
            }
            thread::sleep(Duration::from_millis(250));
        }
        send_key(session, key);
    }
    Err(seen)
}

fn visited(captures: &[String]) -> String {
    captures
        .iter()
        .filter_map(|cap| cap.lines().nth(4))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n  ")
}

fn init_git_repo(dir: &Path) {
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "tripwire")
            .env("GIT_AUTHOR_EMAIL", "tripwire@example.invalid")
            .env("GIT_COMMITTER_NAME", "tripwire")
            .env("GIT_COMMITTER_EMAIL", "tripwire@example.invalid")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "--initial-branch=logtab"]);
    fs::write(dir.join("README.md"), "log tab fixture\n").expect("seed a file");
    git(&["add", "README.md"]);
    git(&["-c", "commit.gpgsign=false", "commit", "-m", "seed"]);
}

fn seed_isolated_home(home: &Path) {
    let base = home.join(".agents-in-a-box");
    let cfg = base.join("config");
    fs::create_dir_all(&cfg).expect("create isolated config dir");
    fs::write(
        cfg.join("onboarding.toml"),
        format!(
            "completed = true\n\
             completed_at = \"2026-09-04T00:00:00+00:00\"\n\
             version = \"{ver}\"\n\
             skipped_dependencies = []\n\
             git_directories = []\n",
            ver = env!("CARGO_PKG_VERSION"),
        ),
    )
    .expect("seed onboarding.toml");
    fs::write(
        base.join("install.json"),
        r#"{"agents":[],"hook_script":"","claude_plugin_dir":null,"codex_hooks_json":null,"plugin_version":null,"prompt_dismissed":true}"#,
    )
    .expect("seed install.json");
}

fn seed_session_registry(home: &Path, tmux_name: &str, worktree: &Path) {
    let entry = json!({
        "sessions": {
            tmux_name: {
                "session_id": "6f1f5f7e-0000-4000-8000-0000000000c1",
                "tmux_session_name": tmux_name,
                "worktree_path": worktree,
                "workspace_name": "logtab",
                "created_at": "2026-09-04T00:00:00Z",
                "agent_type": "Claude",
                "skip_permissions": true,
            }
        }
    });
    fs::write(
        home.join(".agents-in-a-box").join("sessions.json"),
        serde_json::to_vec_pretty(&entry).expect("encode sessions.json"),
    )
    .expect("seed sessions.json");
}

/// A store the size an ainb host actually accumulates.
///
/// The filler rows are the point: this pane shows forty lines, and the read it
/// used to do on every frame had to carry four thousand rows and their payloads
/// to find them.
fn seed_big_store(base: &Path, cwd: &Path) {
    let paths = Paths::under(base);
    fs::create_dir_all(&paths.base).expect("create notifyd base");
    let store = Store::open(&paths.db).expect("open notifications.db");
    let filler = "x".repeat(PAYLOAD_BYTES);
    let now = chrono::Utc::now().timestamp_millis();
    for index in 0..FILLER_ROWS {
        store
            .insert(&Envelope {
                protocol_version: 1,
                agent: "claude".into(),
                raw_event: "Notification:idle_prompt".into(),
                session_id: format!("filler-{index}"),
                // Another worktree entirely: these rows exist to be READ and
                // discarded, which is exactly what the old query did with them.
                cwd: "/tmp/ainb-logtab-filler".into(),
                project: "filler".into(),
                ts: now - 3_600_000 - (index as i64),
                payload: json!({ "message": filler }),
            })
            .expect("seed filler row");
    }
    for index in 0..MINE_ROWS {
        store
            .insert(&Envelope {
                protocol_version: 1,
                agent: "claude".into(),
                raw_event: "Notification:idle_prompt".into(),
                session_id: "logtab-1".into(),
                cwd: cwd.to_string_lossy().into_owned(),
                project: "logtab".into(),
                ts: now - 120_000 - (index as i64),
                payload: json!({ "message": MINE_MESSAGE }),
            })
            .expect("seed session row");
    }
}

#[test]
fn the_log_tab_shows_its_history_without_stalling_the_key_loop() {
    if !tmux_available() {
        eprintln!("SKIP: tmux not available");
        return;
    }

    let home_tmp = tempfile::Builder::new()
        .prefix("ainb-logtab-")
        .tempdir_in("/tmp")
        .expect("home tempdir");
    let home = home_tmp.path();
    seed_isolated_home(home);

    let pid = std::process::id();
    let worktree = home.join("logtab");
    fs::create_dir_all(&worktree).expect("create worktree dir");
    init_git_repo(&worktree);
    let agent_tmux = format!("tmux_logtab_{pid}");
    seed_session_registry(home, &agent_tmux, &worktree);

    // `Paths::from_home` prefers `AINB_HANGAR_HOME` over `$HOME`, and the TUI
    // is launched with it set below — so the store has to be seeded under the
    // SAME base or the pane renders its (correct) empty state forever.
    let hangar_home = home.join("hangar-home");
    fs::create_dir_all(&hangar_home).expect("create isolated hangar home");
    fs::write(
        hangar_home.join("install.json"),
        r#"{"agents":[],"hook_script":"","prompt_dismissed":true}"#,
    )
    .expect("dismiss the notification prompt in the daemon home");
    let seeded_at = Instant::now();
    seed_big_store(&hangar_home, &worktree);
    eprintln!(
        "seeded {FILLER_ROWS} filler + {MINE_ROWS} session rows in {:?}",
        seeded_at.elapsed()
    );

    let _pane = ExactTmuxSession::create(agent_tmux, &["sh", "-c", "sleep 900"]);

    let tui_tmux = format!("tripwire-logtab-{pid}");
    let tui = ExactTmuxSession::create(tui_tmux.clone(), &[]);
    let launch = format!(
        "HOME={home} AINB_HANGAR_HOME={hangar} AINB_DISABLE_PLUGINS=1 exec {bin} tui",
        home = home.display(),
        hangar = hangar_home.display(),
        bin = ainb_bin().display()
    );
    assert!(
        Command::new("tmux")
            .args(["send-keys", "-t", &tui_tmux, &launch, "Enter"])
            .status()
            .expect("launch ainb tui")
            .success(),
        "tmux refused the launch command"
    );

    assert!(
        poll(&tui_tmux, Instant::now() + Duration::from_secs(90), |c| {
            c.contains("Sessions") && c.contains("[s]")
        })
        .is_some(),
        "HomeScreen never rendered:\n{}",
        capture_pane(&tui_tmux)
    );

    assert!(
        poll_until(
            &tui_tmux,
            "s",
            Instant::now() + Duration::from_secs(90),
            |c| c.contains("Workspaces ("),
            |c| c.contains("preview") && c.contains("log"),
        )
        .is_some(),
        "the sessions screen never rendered its tab strip:\n---\n{}\n---",
        capture_pane(&tui_tmux)
    );

    // 1. The pane still does its job. A fix that made the tab fast by making it
    //    empty would pass a timing assertion on its own.
    // BOTH tokens, not just the message: `Notification:idle_prompt` also raises
    // an ASK, and the `ask` pane leads with the same sentence. Only the log
    // pane prints the raw hook event beside it.
    let log_pane = press_until(&tui_tmux, "Tab", 8, |c| {
        c.contains(MINE_MESSAGE) && c.contains(MINE_EVENT)
    })
    .unwrap_or_else(|seen| {
        panic!(
            "Tab never reached a log pane showing this session's history. \
             Panes visited:\n  {}\n---\n{}\n---",
            visited(&seen),
            capture_pane(&tui_tmux)
        )
    });
    assert!(
        !log_pane.contains(&"x".repeat(16)),
        "and only THIS session's rows — the filler payload is on screen:\n{log_pane}"
    );

    // 2. THE responsiveness proof: queue a burst the loop has to repaint its
    //    way through, and time how long the pane takes to answer the key at
    //    the end of it.
    // ONE `send-keys`, not one per key: the clock has to measure the TUI
    // draining the burst, and 200 tmux invocations would put most of a second
    // of process spawning inside the window being asserted on.
    let mut burst = Command::new("tmux");
    burst.args(["send-keys", "-t", &tui_tmux]);
    for _ in 0..BURST_KEYS {
        burst.arg("F5");
    }
    // The one key with a visible effect, queued LAST. Keys are drained in
    // order, so the overlay it opens cannot appear until every repaint ahead of
    // it has happened.
    burst.arg("?");
    assert!(
        burst.status().expect("tmux send-keys burst").success(),
        "tmux refused the burst"
    );
    let burst_started = Instant::now();
    let answered = poll(
        &tui_tmux,
        burst_started + BURST_DEADLINE,
        // The overlay's own border title, so nothing else on a 200-column
        // screen can satisfy this.
        |c| c.contains("Help - Press ? or Esc to close"),
    );
    let elapsed = burst_started.elapsed();
    let last = capture_pane(&tui_tmux);
    drop(tui);
    assert!(
        answered.is_some(),
        "{BURST_KEYS} keystrokes queued on the log tab were not drained within \
         {BURST_DEADLINE:?} ({elapsed:?} elapsed). The pane is reading the \
         notifications store on the render thread again.\n---\n{last}\n---"
    );
    eprintln!("burst of {BURST_KEYS} keys drained in {elapsed:?}");
}
