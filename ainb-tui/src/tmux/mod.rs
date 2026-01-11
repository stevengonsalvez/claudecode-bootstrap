// ABOUTME: Tmux session management module for agents-in-a-box
//
// This module provides tmux-based session management as an alternative to
// Docker containers, enabling:
// - Native tmux sessions for Claude Code interactions
// - Live preview of session output in TUI
// - Seamless attach/detach with Ctrl+Q
// - Scroll mode for reviewing session history
// - Lightweight, fast, and responsive interactions

pub mod capture;
pub mod process_detection;
pub mod pty_wrapper;
pub mod session;

/// Check if reattach-to-user-namespace is available (macOS only).
/// This tool restores access to the macOS user namespace (audio, clipboard, etc.) in tmux.
#[cfg(target_os = "macos")]
pub async fn has_reattach_to_user_namespace() -> bool {
    tokio::process::Command::new("which")
        .arg("reattach-to-user-namespace")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Non-macOS: reattach-to-user-namespace is not needed.
#[cfg(not(target_os = "macos"))]
pub async fn has_reattach_to_user_namespace() -> bool {
    false
}

#[allow(unused_imports)]
pub use capture::CaptureOptions;
pub use process_detection::ClaudeProcessDetector;
#[allow(unused_imports)]
pub use pty_wrapper::PtyWrapper;
#[allow(unused_imports)]
pub use session::{AttachState, TmuxSession};

use anyhow::Result;
use tokio::process::Command;
use tracing::debug;

/// Configure clipboard integration for a tmux session
///
/// Enables OSC 52 clipboard support and platform-specific copy commands
/// so that copy/paste works with the system clipboard.
pub async fn configure_clipboard(session_name: &str) -> Result<()> {
    debug!("Configuring clipboard for tmux session: {}", session_name);

    // Enable set-clipboard for OSC 52 escape sequence support
    // This allows the terminal to access the system clipboard
    let output = Command::new("tmux")
        .args(["set-option", "-t", session_name, "set-clipboard", "on"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to set tmux option set-clipboard: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Configure copy-pipe for mouse selection to use system clipboard
    #[cfg(target_os = "macos")]
    {
        bind_clipboard_for_copy_modes(session_name, "pbcopy").await?;
        debug!("Configured macOS clipboard with pbcopy");
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try xclip first, then xsel
        let copy_cmd = if which::which("xclip").is_ok() {
            Some("xclip -selection clipboard")
        } else if which::which("xsel").is_ok() {
            Some("xsel --clipboard --input")
        } else {
            None
        };

        if let Some(cmd) = copy_cmd {
            bind_clipboard_for_copy_modes(session_name, cmd).await?;
            debug!("Configured Linux clipboard with: {}", cmd);
        } else {
            debug!("No clipboard tool found on Linux (xclip or xsel)");
        }
    }

    Ok(())
}

/// Bind clipboard copy command for both copy-mode and copy-mode-vi
/// Note: tmux key bindings are global, not session-specific
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn bind_clipboard_for_copy_modes(_session_name: &str, copy_cmd: &str) -> Result<()> {
    // Bind for both emacs-style (copy-mode) and vi-style (copy-mode-vi) modes
    for mode in ["copy-mode-vi", "copy-mode"] {
        let output = Command::new("tmux")
            .args([
                "bind-key", "-T", mode, "MouseDragEnd1Pane",
                "send-keys", "-X", "copy-pipe-and-cancel", copy_cmd
            ])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to bind tmux key for {}: {}",
                mode,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(())
}
