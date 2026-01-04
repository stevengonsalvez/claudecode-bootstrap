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
