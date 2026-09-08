//! Tripwire: the ERR window is reachable from BOTH surfaces a user has.
//!
//! `[ui] attention_err_window_hours` decides how long a failure keeps lighting
//! a session row. A knob that only exists in `config.toml` is a knob most
//! operators never find, and a knob that only exists in the settings screen
//! cannot be set by a dotfile or a provisioning script — so it has to be both,
//! and this drives the real binary to prove it:
//!
//!   a seeded `config.toml` value renders on the Settings screen
//!     → `/` finds the row by its dotted key from anywhere in the tree
//!     → `Enter` edits it, the row re-renders with the new value
//!     → `Esc` `Esc` returns to the HomeScreen
//!     → the edit is on disk, and the seeded value is gone
//!
//! Same shape as `tripwire_config_registry_screen.rs`, deliberately: this key
//! is only correctly wired if it behaves exactly like every other registry row.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// The row under test, and its seeded / edited values. Both are inside the
/// registry's declared 1..=8760 range, so the edit exercises validation on the
/// way to disk rather than being rejected before it.
const ROW_KEY: &str = "ui.attention_err_window_hours";
const SEED_HOURS: &str = "3";
const EDITED_HOURS: &str = "9";

const CONFIG_TITLE: &str = "Configuration";
const HOME_MARKER: &str = "Stats";

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

fn seed_isolated_home(home: &Path) -> PathBuf {
    let cfg = home.join(".agents-in-a-box").join("config");
    fs::create_dir_all(&cfg).expect("create config dir");
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

    let config_path = cfg.join("config.toml");
    fs::write(
        &config_path,
        format!("[ui]\nattention_err_window_hours = {SEED_HOURS}\n"),
    )
    .expect("seed config.toml");

    let paths = ainb_plugin_notifyd::Paths::under(home.join(".agents-in-a-box"));
    fs::create_dir_all(&paths.base).expect("create agents-in-a-box base");
    ainb_plugin_notifyd::dismiss_prompt(&paths).expect("seed dismissed notify prompt");

    config_path
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
        thread::sleep(Duration::from_millis(400));
    }
    None
}

/// Re-sends `key` each iteration until `ok` holds. Only safe for idempotent
/// keys — used here for `o`, a no-op once the screen is open.
fn poll_capture_resending<F>(
    session: &str,
    key: &str,
    deadline: Instant,
    mut ok: F,
) -> Option<String>
where
    F: FnMut(&str) -> bool,
{
    send_key(session, key);
    while Instant::now() < deadline {
        let cap = capture_pane(session);
        if ok(&cap) {
            return Some(cap);
        }
        thread::sleep(Duration::from_millis(400));
        send_key(session, key);
    }
    None
}

fn send_key(session: &str, key: &str) {
    Command::new("tmux")
        .args(["send-keys", "-t", session, key])
        .status()
        .expect("tmux send-keys");
}

fn type_literal(session: &str, text: &str) {
    Command::new("tmux")
        .args(["send-keys", "-t", session, "-l", text])
        .status()
        .expect("tmux type literal");
}

fn kill(session: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", &format!("={session}")])
        .status();
}

/// The rendered row for `key`, padding collapsed: `"<key> : <value>"`.
///
/// Asserting on this rather than `capture.contains("9")` is the difference
/// between "the edit landed on this row" and "the digit 9 appears somewhere on
/// a 200-column screen".
fn rendered_row(capture: &str, key: &str) -> Option<String> {
    let line = capture.lines().find(|line| line.contains(key))?;
    let start = line.find(key)?;
    let segment = line[start..].trim_end_matches(['\u{2502}', ' ']);
    Some(segment.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[test]
fn the_err_window_is_settable_from_the_settings_screen_and_lands_in_config_toml() {
    if !tmux_available() {
        eprintln!("SKIP: tmux not available");
        return;
    }

    let home_tmp = tempfile::Builder::new()
        .prefix("ainb-errwindow-")
        .tempdir_in("/tmp")
        .expect("home tempdir");
    let config_path = seed_isolated_home(home_tmp.path());

    let session = format!("tripwire-err-window-{}", std::process::id());
    assert!(
        Command::new("tmux")
            .args(["new-session", "-d", "-s", &session, "-x", "200", "-y", "50"])
            .status()
            .expect("tmux new-session")
            .success(),
        "tmux new-session failed"
    );

    let cmd = format!(
        "HOME={} AINB_DISABLE_PLUGINS=1 exec {} tui",
        home_tmp.path().display(),
        ainb_bin().display()
    );
    Command::new("tmux")
        .args(["send-keys", "-t", &session, &cmd, "Enter"])
        .status()
        .expect("tmux send launch cmd");

    if poll_capture(&session, Instant::now() + Duration::from_secs(60), |c| {
        c.contains(HOME_MARKER) && c.contains("[i]")
    })
    .is_none()
    {
        let last = capture_pane(&session);
        kill(&session);
        panic!("HomeScreen never rendered; last capture:\n---\n{last}\n---");
    }

    if poll_capture_resending(
        &session,
        "o",
        Instant::now() + Duration::from_secs(30),
        |c| c.contains(CONFIG_TITLE),
    )
    .is_none()
    {
        let last = capture_pane(&session);
        kill(&session);
        panic!("Settings never opened; last:\n---\n{last}\n---");
    }

    // 1. The key exists as a registry row and renders the SEEDED value, which
    //    is what proves the TOML half reached the screen rather than the row
    //    rendering its own coded default.
    send_key(&session, "/");
    thread::sleep(Duration::from_millis(300));
    type_literal(&session, "attention_err_window_hours");
    let Some(matched) = poll_capture(&session, Instant::now() + Duration::from_secs(20), |c| {
        c.contains(ROW_KEY)
    }) else {
        let last = capture_pane(&session);
        kill(&session);
        panic!(
            "`/` search did not surface {ROW_KEY}; the knob is not a registry \
             row, so the settings screen can never offer it:\n---\n{last}\n---"
        );
    };
    assert_eq!(
        rendered_row(&matched, ROW_KEY).as_deref(),
        Some(format!("{ROW_KEY} : {SEED_HOURS}").as_str()),
        "the row did not render the value config.toml set:\n---\n{matched}\n---"
    );

    // 2. Edit it from the screen.
    send_key(&session, "Enter");
    if poll_capture(&session, Instant::now() + Duration::from_secs(15), |c| {
        c.contains("Enter save | Esc cancel")
    })
    .is_none()
    {
        let last = capture_pane(&session);
        kill(&session);
        panic!("the edit popup never mounted; last:\n---\n{last}\n---");
    }
    for _ in 0..(SEED_HOURS.len() + 4) {
        send_key(&session, "BSpace");
    }
    type_literal(&session, EDITED_HOURS);
    thread::sleep(Duration::from_millis(200));
    send_key(&session, "Enter");

    let edited = poll_capture(&session, Instant::now() + Duration::from_secs(20), |c| {
        rendered_row(c, ROW_KEY).as_deref() == Some(format!("{ROW_KEY} : {EDITED_HOURS}").as_str())
    });
    if edited.is_none() {
        let last = capture_pane(&session);
        kill(&session);
        panic!(
            "the edited row did not re-render as '{ROW_KEY} : {EDITED_HOURS}'; \
             last:\n---\n{last}\n---"
        );
    }

    // 3. Return path: a forward-only test passes while the way back is broken.
    send_key(&session, "Escape");
    thread::sleep(Duration::from_millis(400));
    send_key(&session, "Escape");
    let back = poll_capture(&session, Instant::now() + Duration::from_secs(20), |c| {
        c.contains(HOME_MARKER) && !c.contains(CONFIG_TITLE)
    });
    let last = capture_pane(&session);
    kill(&session);
    assert!(
        back.is_some(),
        "Esc did not return to the HomeScreen; last:\n---\n{last}\n---"
    );

    // 4. The TOML half, the other direction: the screen's edit is on disk and
    //    the old value is gone — a save that appended would leave both, and the
    //    next launch would read whichever serde saw first.
    let on_disk = fs::read_to_string(&config_path).expect("read seeded config.toml");
    assert!(
        on_disk.contains(&format!("attention_err_window_hours = {EDITED_HOURS}")),
        "config.toml did not persist the edit; contents:\n---\n{on_disk}\n---"
    );
    assert!(
        !on_disk.contains(&format!("attention_err_window_hours = {SEED_HOURS}")),
        "config.toml still carries the old value; contents:\n---\n{on_disk}\n---"
    );
}
