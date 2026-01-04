// ABOUTME: UI components for the TUI interface including session list, logs viewer, and help

pub mod attached_terminal;
pub mod auth_setup;
pub mod claude_chat;
pub mod confirmation_dialog;
pub mod fuzzy_file_finder;
pub mod git_view;
pub mod help;
pub mod layout;
pub mod live_logs_stream;
// pub mod log_formatter;  // Complex version with borrow issues, using simple version instead
pub mod log_formatter_simple;
pub mod log_parser;
pub mod logs_viewer;
pub mod new_session;
pub mod session_list;
pub mod tmux_preview;

pub use attached_terminal::AttachedTerminalComponent;
pub use auth_setup::AuthSetupComponent;
pub use claude_chat::ClaudeChatComponent;
pub use confirmation_dialog::ConfirmationDialogComponent;
pub use git_view::{GitViewComponent, GitViewState};
pub use help::HelpComponent;
pub use layout::LayoutComponent;
pub use live_logs_stream::LiveLogsStreamComponent;
pub use logs_viewer::LogsViewerComponent;
pub use new_session::NewSessionComponent;
pub use session_list::SessionListComponent;
#[allow(unused_imports)]
pub use tmux_preview::{PreviewMode, TmuxPreviewPane};
