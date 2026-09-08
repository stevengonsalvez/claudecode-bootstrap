//! Tripwire: `ERR` stops being a chip that lights forever and says nothing.
//!
//! Two complaints, one screen:
//!
//! 1. "the ERR once it happens is there always even if very old" — a failure
//!    never cleared, so one bad session eventually painted the whole list red
//!    and the chip stopped carrying information;
//! 2. "No way to see the ERR" — the REASON existed (a daemon `error` row's
//!    payload, `SessionStatus::Error`'s own string) and reached no per-session
//!    surface at all.
//!
//! What has to be true on a real screen, and is not provable from a unit test:
//!
//! - a FRESH failure still lights `ERR` on its row;
//! - a failure older than `[ui] attention_err_window_hours` does NOT;
//! - the `err` tab is in the strip, opens on the stale row, and shows the
//!   reason the producer gave;
//! - and it says WHY the row is quiet about it, so retiring the chip does not
//!   simply move the mystery.
//!
//! Runs against a real hangar daemon on an isolated socket, so both error rows
//! are ones that actually travelled the wire.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use ainb_hangar_store::repo::attention::{AttentionKind, AttentionRepo, NewAttention};
use serde_json::json;

#[path = "support/fleet_hangar.rs"]
mod fleet_hangar;

use fleet_hangar::{EnvGuard, FleetHangar};

/// The two failures, and the branch names their rows are labelled with.
///
/// Branch names, because that is what the session list actually prints for an
/// unlabelled session — asserting on a line found by branch is the difference
/// between "this row has no ERR" and "the string ERR is absent from a
/// 200-column screen".
const STALE_BRANCH: &str = "stalefail";
const FRESH_BRANCH: &str = "freshfail";
const STALE_REASON: &str = "worktree vanished under the agent";
const FRESH_REASON: &str = "adapter exited 1: no such model";

/// Comfortably outside the shipped two-hour window, and comfortably inside it.
const STALE_AGE_MS: i64 = 5 * 60 * 60 * 1000;
const FRESH_AGE_MS: i64 = 40_000;

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

/// A tmux session killed by EXACT name on drop. `=name` is the SESSION form.
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
/// Re-checks between presses: a pane can take more than one repaint to settle,
/// and pressing again in that window walks straight past the pane the caller
/// was waiting for. Returns every capture it saw on failure, so the panic says
/// which panes it actually visited.
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
        for _ in 0..4 {
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

/// The right pane's first content line from each capture.
fn visited(captures: &[String]) -> String {
    captures
        .iter()
        .filter_map(|cap| cap.lines().nth(4))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// The one rendered line carrying `branch`, which is what the session list
/// prints for an unlabelled session.
///
/// Skips the Session Info panel below the list: that line also carries the
/// branch, and matching it would test the footer rather than the row.
fn row_for<'a>(capture: &'a str, branch: &str) -> Option<&'a str> {
    let info = session_info_line(capture);
    capture.lines().find(|line| line.contains(branch) && Some(*line) != info)
}

/// The line inside the `Session Info` box, which names the session under the
/// cursor. Found by its box title rather than by a status word, so nothing in
/// the list above can be mistaken for it.
fn session_info_line(capture: &str) -> Option<&str> {
    let mut lines = capture.lines();
    lines.by_ref().find(|line| line.contains("Session Info"))?;
    lines.next()
}

/// Move the cursor down until the Session Info box names `branch`.
///
/// Polled rather than counted: the list interleaves workspace headers with
/// session rows, so "two downs" is a fact about today's fixture and this is a
/// fact about where the cursor actually is.
fn select_session(session: &str, branch: &str) -> Option<String> {
    for _ in 0..12 {
        for _ in 0..4 {
            let cap = capture_pane(session);
            if session_info_line(&cap).is_some_and(|line| line.contains(branch)) {
                return Some(cap);
            }
            thread::sleep(Duration::from_millis(200));
        }
        send_key(session, "Down");
    }
    None
}

fn init_git_repo(dir: &Path, branch: &str) {
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
    git(&["init", &format!("--initial-branch={branch}")]);
    fs::write(dir.join("README.md"), "err pane fixture\n").expect("seed a file");
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

/// Two sessions in one registry: the stale one FIRST, so it is the row under
/// the cursor when the screen opens and the `err` tab can be reached without
/// navigating between rows.
fn seed_session_registry(home: &Path, stale: (&str, &Path), fresh: (&str, &Path)) {
    let entry = |id: &str, tmux: &str, worktree: &Path| {
        json!({
            "session_id": id,
            "tmux_session_name": tmux,
            "worktree_path": worktree,
            "workspace_name": "errpane",
            "created_at": "2026-09-04T00:00:00Z",
            "agent_type": "Claude",
            "skip_permissions": true,
        })
    };
    let registry = json!({
        "sessions": {
            stale.0: entry("6f1f5f7e-0000-4000-8000-0000000000e1", stale.0, stale.1),
            fresh.0: entry("6f1f5f7e-0000-4000-8000-0000000000e2", fresh.0, fresh.1),
        }
    });
    fs::write(
        home.join(".agents-in-a-box").join("sessions.json"),
        serde_json::to_vec_pretty(&registry).expect("encode sessions.json"),
    )
    .expect("seed sessions.json");
}

#[test]
fn a_stale_err_leaves_the_row_and_the_err_pane_says_why() {
    if !tmux_available() {
        eprintln!("SKIP: tmux not available");
        return;
    }

    let home_tmp = tempfile::Builder::new()
        .prefix("ainb-errpane-")
        .tempdir_in("/tmp")
        .expect("home tempdir");
    let home = home_tmp.path();
    seed_isolated_home(home);

    let hangar_home = home.join("hangar-home");
    fs::create_dir_all(&hangar_home).expect("create isolated hangar home");
    fs::write(
        hangar_home.join("install.json"),
        r#"{"agents":[],"hook_script":"","prompt_dismissed":true}"#,
    )
    .expect("dismiss the notification prompt in the daemon home");
    let _hangar_home_guard = EnvGuard::set("AINB_HANGAR_HOME", &hangar_home);
    let hangar = FleetHangar::start(&hangar_home);

    let pid = std::process::id();
    let stale_tree = home.join("stale");
    let fresh_tree = home.join("fresh");
    for (dir, branch) in [(&stale_tree, STALE_BRANCH), (&fresh_tree, FRESH_BRANCH)] {
        fs::create_dir_all(dir).expect("create worktree dir");
        init_git_repo(dir, branch);
    }
    let stale_tmux = format!("tmux_err_stale_{pid}");
    let fresh_tmux = format!("tmux_err_fresh_{pid}");
    seed_session_registry(home, (&stale_tmux, &stale_tree), (&fresh_tmux, &fresh_tree));

    // Two real `error` rows through the daemon, differing only in age. Same
    // producer, same shape, so the ONLY thing that can explain one chip being
    // on the row and the other not is the window.
    let now_ms = chrono::Utc::now().timestamp_millis();
    for (id, tree, reason, age) in [
        ("att-err-stale", &stale_tree, STALE_REASON, STALE_AGE_MS),
        ("att-err-fresh", &fresh_tree, FRESH_REASON, FRESH_AGE_MS),
    ] {
        hangar.block_on(async {
            AttentionRepo::insert(
                hangar.pool(),
                &NewAttention {
                    id: id.to_string(),
                    session_id: format!("provider-{id}"),
                    cwd: tree.to_string_lossy().into_owned(),
                    workspace_id: None,
                    kind: AttentionKind::Error,
                    payload: json!({ "reason": reason }).to_string(),
                    degraded: false,
                    created_at: now_ms - age,
                    raise_transcript: None,
                    channels: ainb_hangar_proto::ChannelSet::default(),
                },
            )
            .await
            .expect("seed the error row");
        });
    }

    let _stale_pane = ExactTmuxSession::create(stale_tmux, &["sh", "-c", "sleep 900"]);
    let _fresh_pane = ExactTmuxSession::create(fresh_tmux, &["sh", "-c", "sleep 900"]);

    let tui_tmux = format!("tripwire-errpane-{pid}");
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

    // Wait for the FRESH row to light. That is also the proof the daemon poll
    // has landed, so a missing chip on the stale row below is the window and
    // not a race with `attention/list`.
    let Some(listed) = poll_until(
        &tui_tmux,
        "s",
        Instant::now() + Duration::from_secs(90),
        |c| c.contains("Workspaces ("),
        |c| row_for(c, FRESH_BRANCH).is_some_and(|row| row.contains("ERR")),
    ) else {
        panic!(
            "the fresh failure never lit an ERR chip, so this test cannot tell a \
             retired chip from a chip that never arrived:\n---\n{}\n---",
            capture_pane(&tui_tmux)
        );
    };

    // THE window proof: same producer, same kind, five hours older, no chip.
    let stale_row = row_for(&listed, STALE_BRANCH)
        .unwrap_or_else(|| panic!("the stale session is not on the screen:\n---\n{listed}\n---"));
    assert!(
        !stale_row.contains("ERR"),
        "a five-hour-old failure must not still be lighting its row: {stale_row}\n\
         ---\n{listed}\n---"
    );

    // The strip carries the new pane without losing the old ones.
    for label in ["preview", "ask", "err", "thread", "copilot", "log"] {
        assert!(
            listed.contains(label),
            "the strip must show every tab, dimmed rather than hidden — `{label}` \
             is missing:\n{listed}"
        );
    }

    // THE reason proof, live half: the cursor opens on the fresh row, and its
    // `err` pane shows the sentence the producer gave — which before this
    // reached no per-session surface at all.
    let Some(_on_fresh) = select_session(&tui_tmux, FRESH_BRANCH) else {
        panic!(
            "the cursor never reached the fresh session:\n---\n{}\n---",
            capture_pane(&tui_tmux)
        );
    };
    let fresh_pane =
        press_until(&tui_tmux, "Tab", 8, |c| c.contains(FRESH_REASON)).unwrap_or_else(|seen| {
            panic!(
                "Tab never reached an err pane showing the fresh reason. Panes \
                 visited:\n  {}\n---\n{}\n---",
                visited(&seen),
                capture_pane(&tui_tmux)
            )
        });
    assert!(
        fresh_pane.contains("ERR"),
        "the err pane names the state it is explaining:\n{fresh_pane}"
    );
    assert!(
        !fresh_pane.contains("no longer lights"),
        "a failure that IS on the row must not claim it has retired:\n{fresh_pane}"
    );

    // THE reason proof, retired half. Same pane, a five-hour-old failure: the
    // reason is still there, and the pane says why the row is quiet about it —
    // so retiring the chip does not simply move the mystery somewhere the
    // operator cannot see it.
    let Some(_on_stale) = select_session(&tui_tmux, STALE_BRANCH) else {
        panic!(
            "the cursor never reached the stale session:\n---\n{}\n---",
            capture_pane(&tui_tmux)
        );
    };
    let stale_pane =
        press_until(&tui_tmux, "Tab", 8, |c| c.contains(STALE_REASON)).unwrap_or_else(|seen| {
            panic!(
                "Tab never reached an err pane showing the stale reason. Panes \
                 visited:\n  {}\n---\n{}\n---",
                visited(&seen),
                capture_pane(&tui_tmux)
            )
        });
    assert!(
        stale_pane.contains("no longer lights"),
        "a retired failure must say why its row is quiet:\n{stale_pane}"
    );
    assert!(
        stale_pane.contains("attention_err_window_hours"),
        "and name the knob that decides it:\n{stale_pane}"
    );

    // Return path: Esc leaves the sessions screen rather than stranding the
    // operator on a pane a forward-only test never had to come back from.
    send_key(&tui_tmux, "Escape");
    let back = poll(&tui_tmux, Instant::now() + Duration::from_secs(20), |c| {
        c.contains("Sessions") && c.contains("[s]") && !c.contains("Workspaces (")
    });
    let last = capture_pane(&tui_tmux);
    drop(tui);
    assert!(
        back.is_some(),
        "Esc did not return to the HomeScreen:\n---\n{last}\n---"
    );
}
