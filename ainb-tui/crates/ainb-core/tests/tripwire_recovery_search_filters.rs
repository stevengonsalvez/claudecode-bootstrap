// ABOUTME: Tripwire for the recovery panel's `/` fuzzy filter.
//
// The recovery panel had no search, so finding one worktree in a long orphan
// list meant scrolling. `/` now filters it. This drives the REAL `ainb tui`
// binary in a detached tmux session, presses the REAL keys, and asserts on the
// REAL painted pane: the unit tests render the component in-process and never
// prove the key ever reaches it.
//
// Isolated HOME: the orphan scan reads `~/.agents-in-a-box/worktrees`, so
// pointing HOME at a tempdir is what stops the developer's own orphans (and
// there are always some) from deciding whether the assertions pass.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// The two orphan worktrees. The query below matches ALPHA as a subsequence
/// and cannot match BETA, so "filtered" and "not filtered" are distinguishable
/// without relying on row order.
const ALPHA: &str = "alpharepo--feature-aaa11111--aaa11111";
const BETA: &str = "betarepo--feature-bbb22222--bbb22222";
const QUERY: &str = "alpha";

fn ainb_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ainb"))
}

fn tmux_available() -> bool {
    Command::new("tmux").arg("-V").output().is_ok_and(|o| o.status.success())
}

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn git_ok(cwd: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args([
            "-c",
            "user.name=ainb-test",
            "-c",
            "user.email=ainb@test.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Skips the first-run wizard. `env!` not a shell grep of Cargo.toml: workspace
/// crates literally carry `version.workspace = true` there.
fn seed_isolated_home(home: &Path) {
    let cfg = home.join(".agents-in-a-box").join("config");
    std::fs::create_dir_all(&cfg).expect("create isolated config dir");
    let onboarding = format!(
        r#"completed = true
completed_at = "2026-05-11T00:00:00+00:00"
version = "{ver}"
skipped_dependencies = []
git_directories = []
"#,
        ver = env!("CARGO_PKG_VERSION"),
    );
    std::fs::write(cfg.join("onboarding.toml"), onboarding).expect("seed onboarding.toml");

    // The notifyd install modal opens over the home screen and EATS the first
    // keypress, which reads exactly like "r does not open recovery". Its paths
    // resolve AINB_HANGAR_HOME before AINB_HOME, so seed both bases, and seed
    // the FULL record: a minimal {"prompt_dismissed": true} fails to parse and
    // falls back to the default, so the modal fires anyway.
    let install = concat!(
        r#"{"agents":[],"hook_script":"","claude_plugin_dir":null,"#,
        r#""codex_hooks_json":null,"copilot_hooks_json":null,"#,
        r#""antigravity_hooks_json":null,"plugin_version":null,"prompt_dismissed":true}"#,
    );
    // `notifyd::Paths::under` puts install.json at the BASE, and the base is
    // `$AINB_HANGAR_HOME`, else `$AINB_HOME`, else `~/.agents-in-a-box`. The
    // launch below sets AINB_HOME to the tempdir itself, so seed both the
    // tempdir root and the home-relative fallback.
    for base in [home.to_path_buf(), home.join(".agents-in-a-box")] {
        std::fs::create_dir_all(&base).expect("create notifyd base");
        std::fs::write(base.join("install.json"), install).expect("seed install.json");
    }
}

/// Two REAL linked worktrees with no tmux session and no sessions.json entry,
/// which is exactly what the scan classifies as orphaned.
fn seed_orphan_worktrees(home: &Path) {
    let repo = home.join("src").join("originrepo");
    std::fs::create_dir_all(&repo).expect("create repo dir");
    assert!(git_ok(&repo, &["init"]), "git init");
    std::fs::write(repo.join("README.md"), "hi").expect("seed README");
    assert!(git_ok(&repo, &["add", "README.md"]), "git add");
    assert!(git_ok(&repo, &["commit", "-m", "init"]), "git commit");

    let by_name = home.join(".agents-in-a-box").join("worktrees").join("by-name");
    std::fs::create_dir_all(&by_name).expect("create by-name dir");

    for (dir, branch) in [(ALPHA, "feature/aaa11111"), (BETA, "feature/bbb22222")] {
        let path = by_name.join(dir);
        assert!(
            git_ok(
                &repo,
                &["worktree", "add", path.to_str().unwrap(), "-b", branch]
            ),
            "git worktree add {dir}"
        );
    }
}

fn capture_pane(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-t", session, "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn poll_capture<F>(session: &str, deadline: Instant, mut ok: F) -> Option<String>
where
    F: FnMut(&str) -> bool,
{
    while Instant::now() < deadline {
        let cap = capture_pane(session);
        if ok(&cap) {
            return Some(cap);
        }
        thread::sleep(Duration::from_millis(500));
    }
    None
}

/// Single keystroke, never with `Enter` appended.
fn send_key(session: &str, key: &str) {
    Command::new("tmux")
        .args(["send-keys", "-t", session, key])
        .status()
        .expect("tmux send-keys");
}

/// Literal text, so `/` and friends are not read as tmux key names.
fn send_text(session: &str, text: &str) {
    Command::new("tmux")
        .args(["send-keys", "-t", session, "-l", text])
        .status()
        .expect("tmux send-keys -l");
}

/// Kills by EXACT session name only. Never kill-server, never a wildcard:
/// other agents' sessions live in this tmux server.
struct TmuxGuard(String);

impl Drop for TmuxGuard {
    fn drop(&mut self) {
        let _ = Command::new("tmux").args(["kill-session", "-t", &self.0]).output();
    }
}

fn deadline(secs: u64) -> Instant {
    Instant::now() + Duration::from_secs(secs)
}

#[test]
fn slash_filters_the_recovery_panel_and_esc_restores_it() {
    if !tmux_available() {
        eprintln!("SKIP: tmux unavailable");
        return;
    }
    if !git_available() {
        eprintln!("SKIP: git unavailable");
        return;
    }

    let home = tempfile::tempdir().expect("tempdir");
    let home_path = home.path().canonicalize().expect("canonicalize home");
    seed_isolated_home(&home_path);
    seed_orphan_worktrees(&home_path);

    let session = format!("tripwire-recovery-search-{}", std::process::id());
    let _ = Command::new("tmux").args(["kill-session", "-t", &session]).output();
    assert!(
        Command::new("tmux")
            .args(["new-session", "-d", "-s", &session, "-x", "180", "-y", "50"])
            .status()
            .is_ok_and(|s| s.success()),
        "failed to create tmux session {session}"
    );
    let _guard = TmuxGuard(session.clone());

    let cmd = format!(
        "HOME={} AINB_HOME={} AINB_DISABLE_PLUGINS=1 exec {} tui",
        home_path.display(),
        home_path.display(),
        ainb_bin().display()
    );
    Command::new("tmux")
        .args(["send-keys", "-t", &session, &cmd, "Enter"])
        .status()
        .expect("launch ainb tui");

    let home_screen = poll_capture(&session, deadline(60), |c| c.contains("Workspaces"))
        .unwrap_or_else(|| panic!("HomeScreen never rendered:\n{}", capture_pane(&session)));
    // Pre-press negative: we are not already looking at the recovery panel, so
    // the assertions after `r` cannot be satisfied by the screen we started on.
    assert!(
        !home_screen.contains("All Orphans"),
        "already on the recovery panel before pressing r:\n{home_screen}"
    );

    // `r` opens Session Recovery from the home screen. It lands on the Sessions
    // tab, which is empty here: the orphans seeded above are worktrees, so Tab
    // once to the Worktrees tab before anything can be asserted about rows.
    send_key(&session, "r");
    poll_capture(&session, deadline(45), |c| c.contains("Worktrees (2)"))
        .unwrap_or_else(|| panic!("recovery panel never opened:\n{}", capture_pane(&session)));
    send_key(&session, "Tab");
    let opened = poll_capture(&session, deadline(30), |c| {
        c.contains(ALPHA) && c.contains(BETA)
    })
    .unwrap_or_else(|| {
        panic!(
            "recovery panel never listed both orphans:\n{}",
            capture_pane(&session)
        )
    });
    assert!(
        opened.contains("/ filter"),
        "the filter affordance is not on screen, so nobody can find the feature:\n{opened}"
    );

    // `/` opens the search bar, then the query narrows the list. Sent as
    // literal text so `/` is not interpreted as a tmux key name.
    send_text(&session, "/");
    send_text(&session, QUERY);
    let filtered = poll_capture(&session, deadline(30), |c| {
        c.contains(ALPHA) && !c.contains(BETA)
    })
    .unwrap_or_else(|| {
        panic!(
            "typing {QUERY:?} did not narrow the list:\n{}",
            capture_pane(&session)
        )
    });
    assert!(
        filtered.contains(QUERY),
        "the search bar does not echo what was typed:\n{filtered}"
    );

    // Esc clears the filter. A forward-only test passes while the way out is
    // broken, which is how a swallowed Esc shipped here before.
    send_key(&session, "Escape");
    let restored = poll_capture(&session, deadline(30), |c| {
        c.contains(ALPHA) && c.contains(BETA)
    })
    .unwrap_or_else(|| {
        panic!(
            "Esc did not restore the unfiltered list:\n{}",
            capture_pane(&session)
        )
    });
    assert!(
        restored.contains("All Orphans") || restored.contains("Worktrees"),
        "Esc left the recovery panel entirely instead of clearing the filter:\n{restored}"
    );
}
