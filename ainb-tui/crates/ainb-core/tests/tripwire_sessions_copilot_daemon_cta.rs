//! Tripwire: the copilot pane names the daemon it needs, offers to start it,
//! and stops advertising a send it cannot perform.
//!
//! Three claims, and only a real screen with a real STOPPED daemon settles any
//! of them:
//!
//! 1. **The cause is the headline, not a suffix.** The reason the copilot did
//!    not open used to arrive tacked onto an unrelated sentence ("no copilot
//!    session yet, nothing to send to · daemon timed out after 5s"), leaving an
//!    operator to already know that "daemon" meant the hangar daemon, that it
//!    can be started, and where.
//! 2. **The remedy is on the pane.** One key, answered in place. Sending the
//!    operator to another screen to find the row that starts it is the dead end
//!    this replaces. INSERTED beside the dial header, never in place of it:
//!    those dials are the other recovery this pane already offered.
//! 3. **The footer stops lying.** `Enter send message` was advertised on a pane
//!    that said "nothing to send to" in the same breath. A surface must never
//!    offer an action it cannot perform.
//!
//! Deliberately UNSEEDED beyond an isolated `$HOME`: no hangar home is
//! requested, so the TUI's own autostart declines the ephemeral home and no
//! daemon exists anywhere. That is exactly the state an operator hits on a
//! fresh install, and it is the only state in which this offer may appear.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// The pane's headline, as `session_tabs::daemon_offer_lines` writes it.
const CTA_HEADLINE: &str = "copilot needs the hangar daemon, which is not running.";
/// The verb the footer and the pane's key line must agree on
/// (`session_tabs::START_DAEMON_VERB`).
const START_VERB: &str = "start the hangar daemon";
/// The footer's old promise, which must be gone from this pane.
const LYING_FOOTER: &str = "Enter send message";

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

/// A tmux session killed by EXACT name on drop.
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

/// Stops whatever daemon this test's own Enter press started, in the home it
/// started it in.
///
/// The offer under test really starts a daemon, so the test really has to take
/// it back down. By `ainb hangar daemon stop` under this `$HOME`, never by
/// killing a pid read from anywhere else: the home is temporary and the stop
/// verb is home-scoped, so nothing outside it can be reached.
struct StopDaemonOnDrop {
    home: PathBuf,
}

impl Drop for StopDaemonOnDrop {
    fn drop(&mut self) {
        let out = Command::new(ainb_bin())
            .args(["hangar", "daemon", "stop"])
            .env("HOME", &self.home)
            .env("AINB_DISABLE_PLUGINS", "1")
            .output();
        if let Ok(out) = out {
            eprintln!(
                "cleanup: hangar daemon stop -> {} {}",
                out.status,
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
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
        thread::sleep(Duration::from_millis(400));
    }
    None
}

/// Press `key` until `arrived`, then stop pressing and keep polling for `ok`.
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

/// Press `key` up to `attempts` times, stopping as soon as `ok` holds.
///
/// Re-checks between presses: the attention poller decides whether this pane
/// shows the offer at all, and it answers a socket rather than a memo, so the
/// pane takes more than one repaint to settle.
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
        for _ in 0..6 {
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
    git(&["init", "--initial-branch=main"]);
    fs::write(dir.join("README.md"), "daemon cta fixture\n").expect("seed a file");
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
    let entry = serde_json::json!({
        "sessions": {
            tmux_name: {
                "session_id": "6f1f5f7e-0000-4000-8000-0000000000d1",
                "tmux_session_name": tmux_name,
                "worktree_path": worktree,
                "workspace_name": "daemoncta",
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

#[test]
fn the_copilot_pane_offers_to_start_the_daemon_it_needs_and_starts_it() {
    if !tmux_available() {
        eprintln!("SKIP: tmux not available");
        return;
    }

    let home_tmp = tempfile::Builder::new()
        .prefix("ainb-dcta-")
        .tempdir_in("/tmp")
        .expect("home tempdir");
    let home = home_tmp.path();
    seed_isolated_home(home);
    // Armed BEFORE the TUI launches, so a daemon started by the key press below
    // is taken back down even if an assertion after it fails.
    let _stop_daemon = StopDaemonOnDrop {
        home: home.to_path_buf(),
    };

    let pid = std::process::id();
    let worktree = home.join("daemoncta");
    fs::create_dir_all(&worktree).expect("create worktree dir");
    init_git_repo(&worktree);
    let agent_tmux = format!("tmux_dcta_{pid}");
    seed_session_registry(home, &agent_tmux, &worktree);
    let _pane = ExactTmuxSession::create(agent_tmux, &["sh", "-c", "sleep 900"]);

    let tui_tmux = format!("tripwire-dcta-{pid}");
    let tui = ExactTmuxSession::create(tui_tmux.clone(), &[]);
    // NO `AINB_HANGAR_HOME`. The home then derives under this temporary `$HOME`
    // and was never asked for, so `ensure_hangar_daemon` declines it and the
    // TUI comes up with no daemon at all — the state under test.
    let launch = format!(
        "HOME={home} AINB_DISABLE_PLUGINS=1 exec {bin} tui",
        home = home.display(),
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
            // The STRIP, not the bare word: the home screen's Recent line
            // carries the workspace name, so a loose match can fire before `s`
            // is ever pressed.
            |c| c.contains("preview") && c.contains("copilot"),
        )
        .is_some(),
        "the sessions screen never rendered:\n{}",
        capture_pane(&tui_tmux)
    );

    // Walk to the copilot tab. Matched on the offer's own headline, because
    // that is the whole claim: this pane must name the hangar daemon, on its
    // own line, rather than trailing it off the end of a sentence about having
    // nothing to send to.
    let offered = press_until(&tui_tmux, "Tab", 8, |c| c.contains(CTA_HEADLINE)).unwrap_or_else(
        |seen| {
            panic!(
                "Tab never reached a copilot pane naming the daemon. Panes visited:\n  {}\n---\n{}\n---",
                seen.iter()
                    .filter_map(|cap| cap.lines().nth(4))
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join("\n  "),
                capture_pane(&tui_tmux)
            )
        },
    );

    // ADDITIVE, and this is the half CI caught when it was not: the offer is
    // INSERTED. The dial header a daemon-down copilot pane already had is still
    // there, with the keys that turn it — those dials are how an operator
    // recovers from an adapter that will not spawn, and replacing them with one
    // sentence takes a working surface away to add another.
    for key in ["\u{2325}e", "\u{2325}o", "\u{2325}g"] {
        assert!(
            offered.contains(key),
            "the offer replaced the dial header instead of joining it: `{key}` gone:\n{offered}"
        );
    }

    // The remedy is ON the pane, with the key next to it.
    assert!(
        offered.contains(START_VERB) && offered.contains('\u{23ce}'),
        "the pane must offer the start with the key that fires it, not merely \
         diagnose:\n{offered}"
    );

    // Claim 3: the footer stops promising a send. Both halves are asserted —
    // the lie is gone AND the verb that replaced it is the one on the pane, so
    // a footer that simply dropped every verb would not pass either.
    assert!(
        !offered.contains(LYING_FOOTER),
        "the footer still advertises `{LYING_FOOTER}` on a pane with nothing to \
         send to:\n{offered}"
    );
    assert!(
        offered.contains("Enter") && offered.contains(START_VERB),
        "the footer must name the verb Enter actually fires here:\n{offered}"
    );

    // THE remedy: one key, answered in place. The proof is the offer going
    // away — the pane only shows it while the poller reports a socket with
    // nothing behind it, so a copilot header appearing here means a daemon
    // really came up and really answered.
    send_key(&tui_tmux, "Enter");
    let settled = poll(&tui_tmux, Instant::now() + Duration::from_secs(90), |c| {
        !c.contains(CTA_HEADLINE) || c.contains('\u{2717}')
    })
    .unwrap_or_else(|| capture_pane(&tui_tmux));
    assert!(
        !settled.contains('\u{2717}'),
        "the start did not land, and the pane says so — which is the honest \
         reporting half working, but the start itself failed:\n{settled}"
    );
    assert!(
        !settled.contains(CTA_HEADLINE),
        "Enter must actually start the daemon, not merely report that it \
         tried:\n{settled}"
    );
    // And what replaces it is the copilot pane proper, dials and all, rather
    // than a blank box.
    let opened = poll(&tui_tmux, Instant::now() + Duration::from_secs(60), |c| {
        c.contains("\u{25c0} \u{2325}e") && c.contains("\u{25c0} \u{2325}g")
    });
    assert!(
        opened.is_some(),
        "the started daemon must hand the operator the real copilot pane:\n{}",
        capture_pane(&tui_tmux)
    );

    drop(tui);
}
