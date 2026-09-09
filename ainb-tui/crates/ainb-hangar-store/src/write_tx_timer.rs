//! Names the code path that holds the single `SQLite` writer too long.
//!
//! ## Why per-statement logging cannot find this
//!
//! `sqlx` already warns on a slow STATEMENT, and that is how the contention was
//! first seen: `card.pull`'s INSERT logging `elapsed=10.64s`. But those lines
//! name the WAITER, never the holder. A transaction that holds the write lock
//! for thirty seconds while issuing three individually-fast statements trips no
//! statement threshold at all — which is precisely why the holder of a real,
//! reproducible, hour-long contention episode could not be identified from a
//! 40 MB daemon log. Hold time is a property of the TRANSACTION, so it has to
//! be measured there.
//!
//! ## Diagnostic only
//!
//! This changes no behaviour: it starts a clock, and on drop it logs if the
//! transaction outlived [`HOLD_WARN`]. Nothing retries, nothing aborts, no
//! statement is reordered. A transaction that behaves costs one `Instant::now`
//! and one comparison, and logs nothing at all.
//!
//! ## What the operator greps for
//!
//! ```text
//! grep hangar.writetx.held ~/.agents-in-a-box/hangar/logs/hangar-daemon.stderr.log
//! ```
//!
//! One literal, present in every such line, chosen so it can be found without
//! knowing a span name in advance.
//!
//! ## What the number means, exactly
//!
//! It is the lifetime of the transaction's SCOPE, which is an UPPER BOUND on
//! the write-lock hold, not the hold itself. `SQLite`'s default `BEGIN` is
//! DEFERRED, so the lock is taken at the first write rather than at `begin()`,
//! and a scope that lingers after `commit()` keeps counting. Both make this
//! read high, never low — which is the right direction for a hunt: it cannot
//! hide a holder, it can only over-report an innocent one.

use std::time::{Duration, Instant};

/// How long a write transaction may live before it is worth a line in the log.
///
/// 10 seconds, matching the pool's `busy_timeout`: a transaction shorter than
/// that cannot have caused another writer to time out, so reporting it would be
/// noise. One that exceeds it is by definition capable of producing the
/// `database is locked` every other writer reports.
pub const HOLD_WARN: Duration = Duration::from_secs(10);

/// Scope guard that reports a write transaction which outlived [`HOLD_WARN`].
///
/// Held alongside the transaction rather than wrapping it, deliberately. The
/// wrapping version would have to re-type 255 `&mut *tx` / `&mut tx` call sites
/// across the data plane, and rewriting the hot write path to diagnose it is
/// how a contention bug gets worse instead of understood.
pub struct WriteTxTimer {
    site: &'static str,
    opened: Instant,
    threshold: Duration,
}

impl WriteTxTimer {
    /// Start timing. `site` should be `concat!(file!(), ":", line!())`.
    #[must_use]
    pub fn start(site: &'static str) -> Self {
        Self::start_with_threshold(site, HOLD_WARN)
    }

    /// [`Self::start`] with an explicit threshold.
    ///
    /// Exists so a test can prove the log line is actually emitted without
    /// sleeping for `busy_timeout`. A ten-second test would be deleted by the
    /// first person to run the suite on a laptop, and an un-run test proves
    /// nothing.
    #[must_use]
    pub fn start_with_threshold(site: &'static str, threshold: Duration) -> Self {
        Self {
            site,
            opened: Instant::now(),
            threshold,
        }
    }
}

impl Drop for WriteTxTimer {
    fn drop(&mut self) {
        let held = self.opened.elapsed();
        if held < self.threshold {
            return;
        }
        // `hangar.writetx.held` is the grep target and must stay literal: it is
        // documented to operators, so renaming it silently breaks the one
        // instruction we give them.
        tracing::warn!(
            target: "hangar.writetx.held",
            site = self.site,
            held_secs = held.as_secs_f64(),
            threshold_secs = self.threshold.as_secs_f64(),
            "hangar.writetx.held: a write transaction held the single SQLite writer longer \
             than busy_timeout; other writers will have reported 'database is locked'"
        );
    }
}

/// Start a [`WriteTxTimer`] labelled with the call site.
///
/// Bind it to a `_`-prefixed local next to the `begin()` it guards, so the
/// timer dies exactly when the transaction's scope does.
#[macro_export]
macro_rules! write_tx_timer {
    () => {
        $crate::write_tx_timer::WriteTxTimer::start(concat!(file!(), ":", line!()))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transaction that behaves logs nothing.
    ///
    /// The hot-path claim: at the daemon's 1 Hz tick the overwhelming majority
    /// of transactions are milliseconds long, and for those this must cost an
    /// `Instant` and a comparison, not a log line.
    #[test]
    fn a_short_transaction_is_not_reported() {
        let timer = WriteTxTimer::start("test-site");
        assert!(
            timer.opened.elapsed() < HOLD_WARN,
            "a freshly started timer cannot already have breached the threshold"
        );
    }

    /// The threshold is the pool's `busy_timeout`, not a taste call.
    ///
    /// A transaction shorter than `busy_timeout` cannot have made another writer
    /// report `database is locked`, so reporting it would be noise; one that
    /// exceeds it is by definition capable of it. If the pool's timeout moves,
    /// this must move with it.
    #[test]
    fn the_threshold_matches_the_pools_busy_timeout() {
        assert_eq!(
            HOLD_WARN,
            Duration::from_secs(10),
            "busy_timeout is 10s in Store::open_in; a different threshold here either \
             hides real holders or reports innocent ones"
        );
    }
}
