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
    Command::new("tmux")
        .args(["set-option", "-t", session_name, "set-clipboard", "on"])
        .status()
        .await?;

    // Configure copy-pipe for mouse selection to use system clipboard
    #[cfg(target_os = "macos")]
    {
        // macOS: Use pbcopy for copy operations
        // This ensures mouse selection copies to system clipboard
        Command::new("tmux")
            .args([
                "bind-key", "-T", "copy-mode-vi", "MouseDragEnd1Pane",
                "send-keys", "-X", "copy-pipe-and-cancel", "pbcopy"
            ])
            .status()
            .await?;

        Command::new("tmux")
            .args([
                "bind-key", "-T", "copy-mode", "MouseDragEnd1Pane",
                "send-keys", "-X", "copy-pipe-and-cancel", "pbcopy"
            ])
            .status()
            .await?;

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
            Command::new("tmux")
                .args([
                    "bind-key", "-T", "copy-mode-vi", "MouseDragEnd1Pane",
                    "send-keys", "-X", "copy-pipe-and-cancel", cmd
                ])
                .status()
                .await?;

            Command::new("tmux")
                .args([
                    "bind-key", "-T", "copy-mode", "MouseDragEnd1Pane",
                    "send-keys", "-X", "copy-pipe-and-cancel", cmd
                ])
                .status()
                .await?;

            debug!("Configured Linux clipboard with: {}", cmd);
        } else {
            debug!("No clipboard tool found on Linux (xclip or xsel)");
        }
    }

    Ok(())
}
