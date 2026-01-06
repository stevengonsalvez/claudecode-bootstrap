// ABOUTME: Core data models for Claude-in-a-Box sessions, workspaces, and state management

pub mod other_tmux;
pub mod session;
pub mod workspace;

pub use other_tmux::OtherTmuxSession;
pub use session::{GitChanges, Session, SessionAgentType, SessionMode, SessionStatus, ShellSession, ShellSessionStatus};
pub use workspace::Workspace;
