// ABOUTME: Event handling system for keyboard input and app actions

#![allow(dead_code)]

use crate::app::{
    AppState,
    state::{AsyncAction, AuthMethod, View},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::info;

// Layout configuration - sessions pane width as percentage of terminal width
const SESSIONS_PANE_WIDTH_PERCENTAGE: f32 = 0.4;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    NextSession,
    PreviousSession,
    NextWorkspace,
    PreviousWorkspace,
    ToggleHelp,
    RefreshWorkspaces, // Manual refresh of workspace data
    ToggleClaudeChat,  // Toggle Claude chat visibility
    NewSession,        // Create session in current directory
    SearchWorkspace,   // Search all workspaces
    AttachSession,
    DetachSession,
    KillContainer,
    ReauthenticateCredentials,
    RestartSession,
    DeleteSession,
    CleanupOrphaned, // Clean up orphaned containers
    SwitchToLogs,
    SwitchToTerminal,
    GoToTop,
    GoToBottom,
    // Pane focus management
    SwitchPaneFocus,
    // Log scrolling events
    ScrollLogsUp,
    ScrollLogsDown,
    ScrollLogsToTop,
    ScrollLogsToBottom,
    ToggleAutoScroll, // Toggle auto-scroll mode in live logs
    // Mouse events
    MouseClick { x: u16, y: u16 },
    MouseDragStart { x: u16, y: u16 },
    MouseDragEnd { x: u16, y: u16 },
    MouseDragging { x: u16, y: u16 },
    // New session creation events
    NewSessionCancel,
    NewSessionNextRepo,
    NewSessionPrevRepo,
    NewSessionConfirmRepo,
    NewSessionInputChar(char),
    NewSessionBackspace,
    NewSessionProceedToModeSelection,
    NewSessionToggleMode,
    NewSessionProceedFromMode,
    NewSessionInputPromptChar(char),
    NewSessionBackspacePrompt,
    NewSessionInsertNewline,
    NewSessionPasteText(String), // Paste text into boss mode prompt
    // Cursor movement events for boss mode prompt
    NewSessionCursorLeft,
    NewSessionCursorRight,
    NewSessionCursorUp,
    NewSessionCursorDown,
    NewSessionCursorLineStart,
    NewSessionCursorLineEnd,
    // Word-wise movement and deletion events
    NewSessionCursorWordLeft,
    NewSessionCursorWordRight,
    NewSessionDeleteWordForward,
    NewSessionDeleteWordBackward,
    NewSessionProceedToPermissions,
    NewSessionTogglePermissions,
    NewSessionCreate,
    // File finder events for @ symbol trigger
    FileFinderNavigateUp,
    FileFinderNavigateDown,
    FileFinderSelectFile,
    FileFinderCancel,
    // Search workspace events
    SearchWorkspaceInputChar(char),
    SearchWorkspaceBackspace,
    // Confirmation dialog events
    ConfirmationToggle,  // Switch between Yes/No
    ConfirmationConfirm, // Confirm action
    ConfirmationCancel,  // Cancel dialog
    // Auth setup events
    AuthSetupNext,            // Next auth method
    AuthSetupPrevious,        // Previous auth method
    AuthSetupSelect,          // Select current method
    AuthSetupCancel,          // Cancel auth setup (skip)
    AuthSetupInputChar(char), // Input character for API key
    AuthSetupBackspace,       // Backspace in API key input
    AuthSetupCheckStatus,     // Check authentication status
    AuthSetupRefresh,         // Manual refresh to check auth completion
    AuthSetupShowCommand,     // Show manual CLI command
    // Git view events
    ShowGitView,       // Show git view for selected session
    GitViewSwitchTab,  // Switch between Files and Diff tabs
    GitViewNextFile,   // Navigate to next file
    GitViewPrevFile,   // Navigate to previous file
    GitViewScrollUp,   // Scroll diff up
    GitViewScrollDown, // Scroll diff down
    GitViewCommitPush, // Commit and push changes
    GitViewBack,       // Return to session list
    GitCommitAndPush,  // Direct commit and push from main view (p key)
    // Quick commit dialog events (for home screen [p] key)
    QuickCommitStart,           // Start quick commit dialog
    QuickCommitInputChar(char), // Character input for quick commit
    QuickCommitBackspace,       // Backspace in quick commit
    QuickCommitCursorLeft,      // Move cursor left
    QuickCommitCursorRight,     // Move cursor right
    QuickCommitConfirm,         // Confirm quick commit (Enter)
    QuickCommitCancel,          // Cancel quick commit (Escape)
    // Commit message input events
    GitViewStartCommit,           // Start commit message input (p key)
    GitViewCommitInputChar(char), // Character input for commit message
    GitViewCommitBackspace,       // Backspace in commit message
    GitViewCommitCursorLeft,      // Move cursor left in commit message
    GitViewCommitCursorRight,     // Move cursor right in commit message
    GitViewCommitCancel,          // Cancel commit message input (Esc)
    GitViewCommitConfirm,         // Confirm and execute commit (Enter)
    GitCommitSuccess(String),     // Commit was successful with message
    // File tree navigation events
    GitViewToggleFolder,          // Toggle folder expand/collapse
    GitViewExpandAll,             // Expand all folders
    GitViewCollapseAll,           // Collapse all folders
    // Tmux integration events
    AttachTmuxSession,            // Attach to tmux session
    DetachTmuxSession,            // Detach from tmux session
    EnterScrollMode,              // Enter scroll mode in tmux preview
    ExitScrollMode,               // Exit scroll mode in tmux preview
    ScrollPreviewUp,              // Scroll tmux preview up
    ScrollPreviewDown,            // Scroll tmux preview down
    ToggleExpandAll,              // Toggle expand/collapse all workspaces
}

pub struct EventHandler;

impl EventHandler {
    /// Handle mouse events and convert to appropriate app events
    pub fn handle_mouse_event(event: AppEvent, state: &mut AppState) -> Option<AppEvent> {
        match event {
            AppEvent::MouseClick { x, y: _ } => {
                // Determine which pane was clicked based on terminal dimensions
                // The layout splits at 40% for sessions, 60% for logs
                let term_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
                let split_point = (term_width as f32 * SESSIONS_PANE_WIDTH_PERCENTAGE) as u16;

                // Check if we're in the main view (not in overlays)
                if state.current_view == View::SessionList && !state.help_visible {
                    if x < split_point {
                        // Clicked in sessions pane
                        if state.focused_pane != crate::app::state::FocusedPane::Sessions {
                            Some(AppEvent::SwitchPaneFocus)
                        } else {
                            None
                        }
                    } else {
                        // Clicked in logs pane
                        if state.focused_pane != crate::app::state::FocusedPane::LiveLogs {
                            Some(AppEvent::SwitchPaneFocus)
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            }
            AppEvent::MouseDragStart { x: _, y: _ } => {
                // Start text selection in logs pane
                if state.focused_pane == crate::app::state::FocusedPane::LiveLogs {
                    // This will be handled in Phase 2
                    None
                } else {
                    None
                }
            }
            AppEvent::MouseDragging { x: _, y: _ } => {
                // Update selection during drag
                if state.focused_pane == crate::app::state::FocusedPane::LiveLogs {
                    // This will be handled in Phase 2
                    None
                } else {
                    None
                }
            }
            AppEvent::MouseDragEnd { x: _, y: _ } => {
                // Finalize text selection
                if state.focused_pane == crate::app::state::FocusedPane::LiveLogs {
                    // This will be handled in Phase 2
                    None
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    /// Get text from system clipboard
    fn get_clipboard_text() -> Result<String, Box<dyn std::error::Error>> {
        use arboard::Clipboard;
        let mut clipboard = Clipboard::new()?;
        let text = clipboard.get_text()?;
        Ok(text)
    }

    pub fn handle_key_event(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        use crate::app::state::View;

        // Handle confirmation dialog first (highest priority)
        if state.confirmation_dialog.is_some() {
            match key_event.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                    return Some(AppEvent::ConfirmationToggle);
                }
                KeyCode::Enter => {
                    return Some(AppEvent::ConfirmationConfirm);
                }
                KeyCode::Esc => {
                    return Some(AppEvent::ConfirmationCancel);
                }
                _ => return None,
            }
        }

        if state.help_visible {
            match key_event.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    return Some(AppEvent::ToggleHelp);
                }
                _ => {
                    return None;
                }
            }
        }

        // Handle global help toggle first (should work from any view)
        if let KeyCode::Char('?') = key_event.code {
            return Some(AppEvent::ToggleHelp);
        }

        // Handle new session creation view
        if state.current_view == View::NewSession {
            return Self::handle_new_session_keys(key_event, state);
        }

        // Handle search workspace view
        if state.current_view == View::SearchWorkspace {
            return Self::handle_search_workspace_keys(key_event, state);
        }

        // Handle non-git notification view
        if state.current_view == View::NonGitNotification {
            return Self::handle_non_git_notification_keys(key_event, state);
        }

        // Handle Claude chat popup view
        if state.current_view == View::ClaudeChat {
            return Self::handle_claude_chat_keys(key_event, state);
        }

        // Handle attached terminal view
        if state.current_view == View::AttachedTerminal {
            return Self::handle_attached_terminal_keys(key_event, state);
        }

        // Handle auth setup view
        if state.current_view == View::AuthSetup {
            return Self::handle_auth_setup_keys(key_event, state);
        }

        // Handle quick commit dialog input
        if state.is_in_quick_commit_mode() {
            return match key_event.code {
                KeyCode::Enter => Some(AppEvent::QuickCommitConfirm),
                KeyCode::Esc => Some(AppEvent::QuickCommitCancel),
                KeyCode::Backspace => Some(AppEvent::QuickCommitBackspace),
                KeyCode::Left => Some(AppEvent::QuickCommitCursorLeft),
                KeyCode::Right => Some(AppEvent::QuickCommitCursorRight),
                KeyCode::Char(ch) => Some(AppEvent::QuickCommitInputChar(ch)),
                _ => None,
            };
        }

        // Handle git view
        if state.current_view == View::GitView {
            tracing::debug!("In git view, handling git view keys");
            return Self::handle_git_view_keys(key_event, state);
        }

        // Handle key events based on focused pane
        use crate::app::state::FocusedPane;

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::Quit),
            KeyCode::Tab => {
                tracing::debug!(
                    "Tab key pressed, current focused_pane: {:?}",
                    state.focused_pane
                );
                Some(AppEvent::SwitchPaneFocus)
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(AppEvent::Quit)
            }
            KeyCode::Char('c') => Some(AppEvent::ToggleClaudeChat),
            KeyCode::Char('f') => Some(AppEvent::RefreshWorkspaces), // Manual refresh
            KeyCode::Char('n') => Some(AppEvent::NewSession),
            KeyCode::Char('s') => Some(AppEvent::SearchWorkspace),
            KeyCode::Char('a') => {
                tracing::info!("[ACTION] 'a' key pressed - AttachTmuxSession requested");
                Some(AppEvent::AttachTmuxSession)
            }
            KeyCode::Char('r') => Some(AppEvent::ReauthenticateCredentials),
            KeyCode::Char('e') => Some(AppEvent::RestartSession),
            KeyCode::Char('d') => Some(AppEvent::DeleteSession),
            KeyCode::Char('x') => Some(AppEvent::CleanupOrphaned),
            KeyCode::Char('g') => Some(AppEvent::ShowGitView), // Show git view
            KeyCode::Char('p') => Some(AppEvent::QuickCommitStart), // Start quick commit dialog
            KeyCode::Char('E') => Some(AppEvent::ToggleExpandAll), // Toggle expand/collapse all workspaces

            // Tmux preview scroll mode (Shift + Up/Down)
            KeyCode::Up if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                Some(AppEvent::ScrollPreviewUp)
            }
            KeyCode::Down if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                Some(AppEvent::ScrollPreviewDown)
            }

            // Navigation keys depend on focused pane
            KeyCode::Char('j') | KeyCode::Down => {
                tracing::debug!("Down key pressed, focused_pane: {:?}", state.focused_pane);
                match state.focused_pane {
                    FocusedPane::Sessions => {
                        tracing::debug!("Sessions pane focused, triggering NextSession");
                        Some(AppEvent::NextSession)
                    }
                    FocusedPane::LiveLogs => {
                        tracing::debug!("LiveLogs pane focused, triggering ScrollLogsDown");
                        Some(AppEvent::ScrollLogsDown)
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                tracing::debug!("Up key pressed, focused_pane: {:?}", state.focused_pane);
                match state.focused_pane {
                    FocusedPane::Sessions => {
                        tracing::debug!("Sessions pane focused, triggering PreviousSession");
                        Some(AppEvent::PreviousSession)
                    }
                    FocusedPane::LiveLogs => {
                        tracing::debug!("LiveLogs pane focused, triggering ScrollLogsUp");
                        Some(AppEvent::ScrollLogsUp)
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                tracing::debug!("Left key pressed, focused_pane: {:?}", state.focused_pane);
                match state.focused_pane {
                    FocusedPane::Sessions => {
                        tracing::debug!("Sessions pane focused, triggering PreviousWorkspace");
                        Some(AppEvent::PreviousWorkspace)
                    }
                    FocusedPane::LiveLogs => {
                        tracing::debug!("LiveLogs pane focused, no left/right scrolling");
                        None // No left/right scrolling in logs
                    }
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                tracing::debug!("Right key pressed, focused_pane: {:?}", state.focused_pane);
                match state.focused_pane {
                    FocusedPane::Sessions => {
                        tracing::debug!("Sessions pane focused, triggering NextWorkspace");
                        Some(AppEvent::NextWorkspace)
                    }
                    FocusedPane::LiveLogs => {
                        tracing::debug!("LiveLogs pane focused, no left/right scrolling");
                        None // No left/right scrolling in logs
                    }
                }
            }
            KeyCode::Home => match state.focused_pane {
                FocusedPane::Sessions => Some(AppEvent::GoToTop),
                FocusedPane::LiveLogs => Some(AppEvent::ScrollLogsToTop),
            },
            KeyCode::End => match state.focused_pane {
                FocusedPane::Sessions => Some(AppEvent::GoToBottom),
                FocusedPane::LiveLogs => Some(AppEvent::ScrollLogsToBottom),
            },
            KeyCode::Char(' ') => match state.focused_pane {
                FocusedPane::Sessions => None, // Space does nothing in sessions pane
                FocusedPane::LiveLogs => Some(AppEvent::ToggleAutoScroll),
            },
            _ => None,
        }
    }

    fn handle_search_workspace_keys(
        key_event: KeyEvent,
        _state: &mut AppState,
    ) -> Option<AppEvent> {
        match key_event.code {
            KeyCode::Esc => Some(AppEvent::NewSessionCancel),
            KeyCode::Down => Some(AppEvent::NewSessionNextRepo),
            KeyCode::Up => Some(AppEvent::NewSessionPrevRepo),
            KeyCode::Enter => Some(AppEvent::NewSessionConfirmRepo),
            KeyCode::Backspace => Some(AppEvent::SearchWorkspaceBackspace),
            KeyCode::Char(ch) => Some(AppEvent::SearchWorkspaceInputChar(ch)),
            _ => None,
        }
    }

    fn handle_new_session_keys(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        use crate::app::state::NewSessionStep;

        if let Some(ref session_state) = state.new_session_state {
            match session_state.step {
                NewSessionStep::SelectRepo => match key_event.code {
                    KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                    KeyCode::Down => Some(AppEvent::NewSessionNextRepo),
                    KeyCode::Up => Some(AppEvent::NewSessionPrevRepo),
                    KeyCode::Enter => Some(AppEvent::NewSessionConfirmRepo),
                    _ => None,
                },
                NewSessionStep::InputBranch => {
                    match key_event.code {
                        KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                        KeyCode::Enter => {
                            // Check if we're in current directory mode
                            if let Some(ref session_state) = state.new_session_state {
                                if session_state.is_current_dir_mode {
                                    // Skip mode selection and permissions for current directory mode
                                    Some(AppEvent::NewSessionCreate)
                                } else {
                                    Some(AppEvent::NewSessionProceedToModeSelection)
                                }
                            } else {
                                Some(AppEvent::NewSessionProceedToModeSelection)
                            }
                        }
                        KeyCode::Backspace => Some(AppEvent::NewSessionBackspace),
                        KeyCode::Char(ch) => Some(AppEvent::NewSessionInputChar(ch)),
                        _ => None,
                    }
                }
                NewSessionStep::SelectMode => match key_event.code {
                    KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                    KeyCode::Enter => Some(AppEvent::NewSessionProceedFromMode),
                    KeyCode::Down | KeyCode::Up => Some(AppEvent::NewSessionToggleMode),
                    _ => None,
                },
                NewSessionStep::InputPrompt => {
                    // Debug logging to understand what key events we're receiving
                    tracing::debug!(
                        "InputPrompt: Received key event: {:?} with modifiers: {:?}",
                        key_event.code,
                        key_event.modifiers
                    );

                    // Check if file finder is active first
                    let file_finder_active =
                        if let Some(ref session_state) = state.new_session_state {
                            session_state.file_finder.is_active
                        } else {
                            false
                        };

                    if file_finder_active {
                        // File finder navigation takes precedence
                        match key_event.code {
                            KeyCode::Esc => {
                                tracing::debug!(
                                    "InputPrompt: Escape pressed while file finder active, cancelling file finder"
                                );
                                Some(AppEvent::FileFinderCancel)
                            }
                            KeyCode::Up => {
                                tracing::debug!("InputPrompt: Up navigation in file finder");
                                Some(AppEvent::FileFinderNavigateUp)
                            }
                            KeyCode::Down => {
                                tracing::debug!("InputPrompt: Down navigation in file finder");
                                Some(AppEvent::FileFinderNavigateDown)
                            }
                            KeyCode::Enter => {
                                tracing::debug!(
                                    "InputPrompt: Enter pressed in file finder, selecting file"
                                );
                                Some(AppEvent::FileFinderSelectFile)
                            }
                            KeyCode::Backspace => {
                                tracing::debug!("InputPrompt: Backspace pressed in file finder");
                                Some(AppEvent::NewSessionBackspacePrompt)
                            }
                            KeyCode::Char(ch) => {
                                tracing::debug!(
                                    "InputPrompt: Character '{}' typed in file finder",
                                    ch
                                );
                                Some(AppEvent::NewSessionInputPromptChar(ch))
                            }
                            _ => {
                                tracing::debug!(
                                    "InputPrompt: Unhandled key in file finder: {:?}",
                                    key_event.code
                                );
                                None
                            }
                        }
                    } else {
                        // Normal prompt input handling
                        match key_event.code {
                            KeyCode::Esc => {
                                tracing::debug!("InputPrompt: Escape pressed, cancelling session");
                                Some(AppEvent::NewSessionCancel)
                            }
                            KeyCode::Enter => {
                                tracing::debug!(
                                    "InputPrompt: Enter detected, checking prompt validity"
                                );
                                // Check if prompt is not empty before proceeding
                                if let Some(ref session_state) = state.new_session_state {
                                    let prompt_string = session_state.boss_prompt.to_string();
                                    let prompt_content = prompt_string.trim();
                                    tracing::debug!(
                                        "InputPrompt: Current prompt content: '{}' (length: {})",
                                        prompt_content,
                                        prompt_content.len()
                                    );
                                    if prompt_content.is_empty() {
                                        tracing::warn!(
                                            "InputPrompt: Prompt is empty, not proceeding"
                                        );
                                        None // Don't proceed if prompt is empty
                                    } else {
                                        tracing::info!(
                                            "InputPrompt: Prompt is valid ({}), proceeding to permissions",
                                            prompt_content.len()
                                        );
                                        Some(AppEvent::NewSessionProceedToPermissions)
                                    }
                                } else {
                                    tracing::error!(
                                        "InputPrompt: No session state found, cannot proceed"
                                    );
                                    None
                                }
                            }
                            KeyCode::Char('j')
                                if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                tracing::debug!("InputPrompt: Ctrl+J pressed, inserting newline");
                                Some(AppEvent::NewSessionInsertNewline)
                            }
                            KeyCode::Char('v')
                                if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                tracing::debug!(
                                    "InputPrompt: Ctrl+V pressed, attempting to paste from clipboard"
                                );
                                // Try to get clipboard content
                                match Self::get_clipboard_text() {
                                    Ok(text) => {
                                        tracing::debug!(
                                            "InputPrompt: Successfully got clipboard text: {} chars",
                                            text.len()
                                        );
                                        Some(AppEvent::NewSessionPasteText(text))
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "InputPrompt: Failed to get clipboard content: {}",
                                            e
                                        );
                                        None
                                    }
                                }
                            }
                            // Option key combinations for word movement and deletion (must come first)
                            KeyCode::Left if key_event.modifiers.contains(KeyModifiers::ALT) => {
                                tracing::debug!("InputPrompt: Option+Left - word left");
                                Some(AppEvent::NewSessionCursorWordLeft)
                            }
                            KeyCode::Right if key_event.modifiers.contains(KeyModifiers::ALT) => {
                                tracing::debug!("InputPrompt: Option+Right - word right");
                                Some(AppEvent::NewSessionCursorWordRight)
                            }
                            KeyCode::Delete if key_event.modifiers.contains(KeyModifiers::ALT) => {
                                tracing::debug!("InputPrompt: Option+Delete - delete word forward");
                                Some(AppEvent::NewSessionDeleteWordForward)
                            }
                            KeyCode::Backspace
                                if key_event.modifiers.contains(KeyModifiers::ALT) =>
                            {
                                tracing::debug!(
                                    "InputPrompt: Option+Backspace - delete word backward"
                                );
                                Some(AppEvent::NewSessionDeleteWordBackward)
                            }
                            KeyCode::Backspace => {
                                tracing::debug!("InputPrompt: Backspace pressed");
                                Some(AppEvent::NewSessionBackspacePrompt)
                            }
                            // Arrow keys only for cursor movement (removed hjkl to allow typing those letters)
                            KeyCode::Left => {
                                tracing::debug!("InputPrompt: Cursor left");
                                Some(AppEvent::NewSessionCursorLeft)
                            }
                            KeyCode::Right => {
                                tracing::debug!("InputPrompt: Cursor right");
                                Some(AppEvent::NewSessionCursorRight)
                            }
                            KeyCode::Up => {
                                tracing::debug!("InputPrompt: Cursor up");
                                Some(AppEvent::NewSessionCursorUp)
                            }
                            KeyCode::Down => {
                                tracing::debug!("InputPrompt: Cursor down");
                                Some(AppEvent::NewSessionCursorDown)
                            }
                            KeyCode::Char(ch) => {
                                tracing::debug!("InputPrompt: Character '{}' typed", ch);
                                Some(AppEvent::NewSessionInputPromptChar(ch))
                            }
                            _ => {
                                tracing::debug!("InputPrompt: Unhandled key: {:?}", key_event.code);
                                None
                            }
                        }
                    }
                }
                NewSessionStep::ConfigurePermissions => {
                    tracing::debug!(
                        "ConfigurePermissions: Received key event: {:?}",
                        key_event.code
                    );
                    match key_event.code {
                        KeyCode::Esc => {
                            tracing::debug!(
                                "ConfigurePermissions: Escape pressed, cancelling session"
                            );
                            Some(AppEvent::NewSessionCancel)
                        }
                        KeyCode::Enter => {
                            tracing::info!(
                                "ConfigurePermissions: Enter pressed, creating new session"
                            );
                            Some(AppEvent::NewSessionCreate)
                        }
                        KeyCode::Char(' ') => {
                            tracing::debug!(
                                "ConfigurePermissions: Space pressed, toggling permissions"
                            );
                            Some(AppEvent::NewSessionTogglePermissions)
                        }
                        _ => {
                            tracing::debug!(
                                "ConfigurePermissions: Unhandled key: {:?}",
                                key_event.code
                            );
                            None
                        }
                    }
                }
                NewSessionStep::Creating => {
                    // During creation, only allow cancellation
                    match key_event.code {
                        KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                        _ => None,
                    }
                }
            }
        } else {
            None
        }
    }

    fn handle_non_git_notification_keys(
        key_event: KeyEvent,
        _state: &mut AppState,
    ) -> Option<AppEvent> {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::Quit),
            KeyCode::Char('s') => Some(AppEvent::SearchWorkspace),
            _ => None,
        }
    }

    fn handle_attached_terminal_keys(
        key_event: KeyEvent,
        _state: &mut AppState,
    ) -> Option<AppEvent> {
        match key_event.code {
            KeyCode::Char('d') => Some(AppEvent::DetachSession),
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::DetachSession),
            KeyCode::Char('k') => Some(AppEvent::KillContainer),
            _ => None, // All other keys are passed through to the terminal
        }
    }

    fn handle_claude_chat_keys(key_event: KeyEvent, _state: &mut AppState) -> Option<AppEvent> {
        match key_event.code {
            // Escape closes the Claude chat popup
            KeyCode::Esc => Some(AppEvent::ToggleClaudeChat),
            // Enter sends the message
            KeyCode::Enter => {
                // TODO: Add send message event
                None
            }
            // Backspace for editing input
            KeyCode::Backspace => {
                // TODO: Add backspace handling
                None
            }
            // All other characters are input to the chat
            KeyCode::Char(_ch) => {
                // TODO: Add character input handling
                None
            }
            _ => None,
        }
    }

    fn handle_auth_setup_keys(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        if let Some(ref auth_state) = state.auth_setup_state {
            // If we're inputting API key, handle text input
            if auth_state.selected_method == AuthMethod::ApiKey
                && !auth_state.api_key_input.is_empty()
            {
                match key_event.code {
                    KeyCode::Enter => Some(AppEvent::AuthSetupSelect),
                    KeyCode::Backspace => Some(AppEvent::AuthSetupBackspace),
                    KeyCode::Esc => Some(AppEvent::AuthSetupBackspace), // Clear input
                    KeyCode::Char(ch) => Some(AppEvent::AuthSetupInputChar(ch)),
                    _ => None,
                }
            } else {
                // Method selection mode or waiting for auth completion
                match key_event.code {
                    KeyCode::Esc => Some(AppEvent::AuthSetupCancel),
                    KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::AuthSetupPrevious),
                    KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::AuthSetupNext),
                    KeyCode::Enter => Some(AppEvent::AuthSetupSelect),
                    KeyCode::Char('r') => Some(AppEvent::AuthSetupRefresh), // Manual refresh
                    KeyCode::Char('c') => Some(AppEvent::AuthSetupShowCommand), // Show CLI command
                    _ => None,
                }
            }
        } else {
            None
        }
    }

    fn handle_git_view_keys(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        tracing::debug!("Git view key pressed: {:?}", key_event);

        // Check if we're in commit message input mode
        let in_commit_mode = if let Some(ref git_state) = state.git_view_state {
            git_state.is_in_commit_mode()
        } else {
            tracing::warn!("No git state available in handle_git_view_keys");
            false
        };

        if in_commit_mode {
            // Handle commit message input
            match key_event.code {
                KeyCode::Esc => Some(AppEvent::GitViewCommitCancel),
                KeyCode::Enter => Some(AppEvent::GitViewCommitConfirm),
                KeyCode::Backspace => Some(AppEvent::GitViewCommitBackspace),
                KeyCode::Left => Some(AppEvent::GitViewCommitCursorLeft),
                KeyCode::Right => Some(AppEvent::GitViewCommitCursorRight),
                KeyCode::Char(ch) => Some(AppEvent::GitViewCommitInputChar(ch)),
                _ => None,
            }
        } else {
            // Normal git view navigation
            match key_event.code {
                KeyCode::Esc => Some(AppEvent::GitViewBack),
                KeyCode::Tab => Some(AppEvent::GitViewSwitchTab),
                KeyCode::Char('j') | KeyCode::Down => {
                    if let Some(ref git_state) = state.git_view_state {
                        match git_state.active_tab {
                            crate::components::git_view::GitTab::Files => {
                                Some(AppEvent::GitViewNextFile)
                            }
                            crate::components::git_view::GitTab::Diff => {
                                Some(AppEvent::GitViewScrollDown)
                            }
                            crate::components::git_view::GitTab::Markdown => {
                                Some(AppEvent::GitViewScrollDown)
                            }
                        }
                    } else {
                        None
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if let Some(ref git_state) = state.git_view_state {
                        match git_state.active_tab {
                            crate::components::git_view::GitTab::Files => {
                                Some(AppEvent::GitViewPrevFile)
                            }
                            crate::components::git_view::GitTab::Diff => {
                                Some(AppEvent::GitViewScrollUp)
                            }
                            crate::components::git_view::GitTab::Markdown => {
                                Some(AppEvent::GitViewScrollUp)
                            }
                        }
                    } else {
                        None
                    }
                }
                KeyCode::Enter => {
                    // Toggle folder on Enter key in Files tab
                    if let Some(ref git_state) = state.git_view_state {
                        if git_state.active_tab == crate::components::git_view::GitTab::Files {
                            Some(AppEvent::GitViewToggleFolder)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                KeyCode::Char('e') => {
                    // Expand all folders
                    if let Some(ref git_state) = state.git_view_state {
                        if git_state.active_tab == crate::components::git_view::GitTab::Files {
                            Some(AppEvent::GitViewExpandAll)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                KeyCode::Char('E') => {
                    // Collapse all folders
                    if let Some(ref git_state) = state.git_view_state {
                        if git_state.active_tab == crate::components::git_view::GitTab::Files {
                            Some(AppEvent::GitViewCollapseAll)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                KeyCode::Char('p') => {
                    tracing::info!("Git view 'p' key pressed - starting commit");
                    Some(AppEvent::GitViewStartCommit)
                }
                _ => None,
            }
        }
    }

    pub fn process_event(event: AppEvent, state: &mut AppState) {
        match event {
            AppEvent::Quit => state.quit(),
            AppEvent::ToggleHelp => state.toggle_help(),
            AppEvent::ToggleClaudeChat => state.toggle_claude_chat(),
            AppEvent::ToggleExpandAll => state.toggle_expand_all_workspaces(),
            AppEvent::RefreshWorkspaces => {
                // Mark for async processing to reload workspace data
                state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
            }
            AppEvent::NextSession => state.next_session(),
            AppEvent::PreviousSession => state.previous_session(),
            AppEvent::NextWorkspace => state.next_workspace(),
            AppEvent::PreviousWorkspace => state.previous_workspace(),
            AppEvent::GoToTop => {
                if state.selected_workspace_index.is_some() {
                    state.selected_session_index = Some(0);
                }
            }
            AppEvent::GoToBottom => {
                if let Some(workspace_idx) = state.selected_workspace_index {
                    if let Some(workspace) = state.workspaces.get(workspace_idx) {
                        if !workspace.sessions.is_empty() {
                            state.selected_session_index = Some(workspace.sessions.len() - 1);
                        }
                    }
                }
            }
            AppEvent::NewSession => {
                // Mark for async processing - create normal new session with mode selection
                state.pending_async_action = Some(AsyncAction::NewSessionNormal);
            }
            AppEvent::SearchWorkspace => {
                // Don't overwrite pending DeleteSession actions
                if let Some(AsyncAction::DeleteSession(_)) = state.pending_async_action {
                    return;
                }

                // Mark for async processing - search all workspaces
                state.pending_async_action = Some(AsyncAction::StartWorkspaceSearch);
                // Clear any previous cancellation flag
                state.async_operation_cancelled = false;
            }
            AppEvent::NewSessionCancel => {
                state.cancel_new_session();
            }
            AppEvent::NewSessionNextRepo => state.new_session_next_repo(),
            AppEvent::NewSessionPrevRepo => state.new_session_prev_repo(),
            AppEvent::NewSessionConfirmRepo => {
                tracing::info!("Event: NewSessionConfirmRepo");
                state.new_session_confirm_repo();
            }
            AppEvent::NewSessionInputChar(ch) => {
                tracing::debug!("Event: NewSessionInputChar({})", ch);
                state.new_session_update_branch(ch);
            }
            AppEvent::NewSessionBackspace => {
                tracing::debug!("Event: NewSessionBackspace");
                state.new_session_backspace();
            }
            AppEvent::NewSessionProceedToModeSelection => {
                tracing::info!("Event: NewSessionProceedToModeSelection");
                state.new_session_proceed_to_mode_selection();
            }
            AppEvent::NewSessionToggleMode => {
                tracing::info!("Event: NewSessionToggleMode");
                state.new_session_toggle_mode();
            }
            AppEvent::NewSessionProceedFromMode => {
                tracing::info!("Event: NewSessionProceedFromMode");
                state.new_session_proceed_from_mode();
            }
            AppEvent::NewSessionInputPromptChar(ch) => state.new_session_add_char_to_prompt(ch),
            AppEvent::NewSessionBackspacePrompt => state.new_session_backspace_prompt(),
            AppEvent::NewSessionInsertNewline => state.new_session_insert_newline(),
            AppEvent::NewSessionPasteText(text) => state.new_session_paste_text(text),
            AppEvent::NewSessionCursorLeft => state.new_session_move_cursor_left(),
            AppEvent::NewSessionCursorRight => state.new_session_move_cursor_right(),
            AppEvent::NewSessionCursorUp => state.new_session_move_cursor_up(),
            AppEvent::NewSessionCursorDown => state.new_session_move_cursor_down(),
            AppEvent::NewSessionCursorLineStart => state.new_session_move_to_line_start(),
            AppEvent::NewSessionCursorLineEnd => state.new_session_move_to_line_end(),
            // Word movement and deletion events
            AppEvent::NewSessionCursorWordLeft => state.new_session_move_cursor_word_left(),
            AppEvent::NewSessionCursorWordRight => state.new_session_move_cursor_word_right(),
            AppEvent::NewSessionDeleteWordForward => state.new_session_delete_word_forward(),
            AppEvent::NewSessionDeleteWordBackward => state.new_session_delete_word_backward(),
            AppEvent::NewSessionProceedToPermissions => {
                tracing::info!("Processing NewSessionProceedToPermissions event");
                state.new_session_proceed_to_permissions();
            }
            AppEvent::NewSessionTogglePermissions => state.new_session_toggle_permissions(),
            AppEvent::NewSessionCreate => {
                tracing::info!("Processing NewSessionCreate event - queueing async action");
                // Mark for async processing
                state.pending_async_action = Some(AsyncAction::CreateNewSession);
            }
            AppEvent::SearchWorkspaceInputChar(ch) => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.filter_text.push(ch);
                    session_state.apply_filter();
                }
            }
            AppEvent::SearchWorkspaceBackspace => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.filter_text.pop();
                    session_state.apply_filter();
                }
            }
            AppEvent::AttachSession => {
                if let Some(session_id) = state.get_selected_session_id() {
                    state.pending_async_action = Some(AsyncAction::AttachToContainer(session_id));
                }
            }
            AppEvent::AttachTmuxSession => {
                tracing::info!("[ACTION] Processing AttachTmuxSession event");
                tracing::debug!(
                    "[ACTION] State: workspace_idx={:?}, session_idx={:?}, is_other_tmux={}, other_tmux_idx={:?}",
                    state.selected_workspace_index,
                    state.selected_session_index,
                    state.is_other_tmux_selected(),
                    state.selected_other_tmux_index
                );

                // Check if we're in the "Other tmux" section
                if state.is_other_tmux_selected() {
                    if let Some(other_session) = state.selected_other_tmux_session() {
                        let session_name = other_session.name.clone();
                        tracing::info!("[ACTION] Attaching to other tmux session: {}", session_name);
                        state.pending_async_action = Some(AsyncAction::AttachToOtherTmux(session_name));
                    } else {
                        tracing::warn!("[ACTION] Other tmux selected but no session found");
                    }
                } else if let Some(session_id) = state.get_selected_session_id() {
                    // Get more info about the session for logging
                    if let Some(session) = state.get_selected_session() {
                        tracing::info!(
                            "[ACTION] Attaching to session: id={}, name={}, tmux_name={:?}, status={:?}",
                            session_id,
                            session.name,
                            session.tmux_session_name,
                            session.status
                        );
                    }
                    state.pending_async_action = Some(AsyncAction::AttachToTmuxSession(session_id));
                } else {
                    tracing::warn!("[ACTION] AttachTmuxSession: No session selected (workspace_idx={:?}, session_idx={:?})",
                        state.selected_workspace_index, state.selected_session_index);
                    state.add_error_notification("No session selected to attach".to_string());
                }
            }
            AppEvent::DetachSession => {
                // Clear attached session and return to session list
                state.attached_session_id = None;
                state.current_view = View::SessionList;
                state.ui_needs_refresh = true;
            }
            AppEvent::DetachTmuxSession => {
                // Detaching from tmux is handled by AttachHandler (Ctrl+Q)
                // This event is a no-op placeholder
                tracing::debug!("DetachTmuxSession event received (no-op)");
            }
            AppEvent::ScrollPreviewUp => {
                // Scroll events are handled by the LayoutComponent's tmux_preview
                // This is a signal that should be processed in main loop
                tracing::debug!("ScrollPreviewUp event (handled by layout component)");
                state.ui_needs_refresh = true;
            }
            AppEvent::ScrollPreviewDown => {
                // Scroll events are handled by the LayoutComponent's tmux_preview
                // This is a signal that should be processed in main loop
                tracing::debug!("ScrollPreviewDown event (handled by layout component)");
                state.ui_needs_refresh = true;
            }
            AppEvent::EnterScrollMode => {
                tracing::debug!("EnterScrollMode event (handled by layout component)");
                state.ui_needs_refresh = true;
            }
            AppEvent::ExitScrollMode => {
                tracing::debug!("ExitScrollMode event (handled by layout component)");
                state.ui_needs_refresh = true;
            }
            AppEvent::KillContainer => {
                if let Some(session_id) = state.attached_session_id {
                    state.pending_async_action = Some(AsyncAction::KillContainer(session_id));
                }
            }
            AppEvent::ReauthenticateCredentials => {
                info!("Queueing re-authentication request");
                state.pending_async_action = Some(AsyncAction::ReauthenticateCredentials);
            }
            AppEvent::RestartSession => {
                if let Some(session_id) = state.get_selected_session_id() {
                    state.pending_async_action = Some(AsyncAction::RestartSession(session_id));
                }
            }
            AppEvent::DeleteSession => {
                // Check if we're in the "Other tmux" section
                if state.is_other_tmux_selected() {
                    if let Some(other_session) = state.selected_other_tmux_session() {
                        state.show_kill_other_tmux_confirmation(other_session.name.clone());
                    }
                } else if let Some(session) = state.selected_session() {
                    // Show confirmation dialog for regular session
                    state.show_delete_confirmation(session.id);
                }
            }
            AppEvent::CleanupOrphaned => {
                // Queue cleanup of orphaned containers
                state.pending_async_action = Some(AsyncAction::CleanupOrphaned);
            }
            AppEvent::SwitchToLogs => {
                // TODO: Implement view switching
            }
            AppEvent::SwitchToTerminal => {
                // TODO: Implement terminal view
            }
            AppEvent::SwitchPaneFocus => {
                use crate::app::state::FocusedPane;
                let old_pane = state.focused_pane.clone();
                state.focused_pane = match state.focused_pane {
                    FocusedPane::Sessions => FocusedPane::LiveLogs,
                    FocusedPane::LiveLogs => FocusedPane::Sessions,
                };
                tracing::debug!(
                    "Switched focus from {:?} to {:?}",
                    old_pane,
                    state.focused_pane
                );
            }
            AppEvent::ScrollLogsUp => {
                // Handled in main.rs to access layout component
            }
            AppEvent::ScrollLogsDown => {
                // Handled in main.rs to access layout component
            }
            AppEvent::ScrollLogsToTop => {
                // Handled in main.rs to access layout component
            }
            AppEvent::ScrollLogsToBottom => {
                // Handled in main.rs to access layout component
            }
            AppEvent::ToggleAutoScroll => {
                // Handled in main.rs to access layout component
            }
            AppEvent::ConfirmationToggle => {
                if let Some(ref mut dialog) = state.confirmation_dialog {
                    dialog.selected_option = !dialog.selected_option;
                }
            }
            AppEvent::ConfirmationConfirm => {
                if let Some(dialog) = state.confirmation_dialog.take() {
                    if dialog.selected_option {
                        // User confirmed, execute the action
                        match dialog.confirm_action {
                            crate::app::state::ConfirmAction::DeleteSession(session_id) => {
                                state.pending_async_action =
                                    Some(AsyncAction::DeleteSession(session_id));
                            }
                            crate::app::state::ConfirmAction::KillOtherTmux(session_name) => {
                                state.pending_async_action =
                                    Some(AsyncAction::KillOtherTmux(session_name));
                            }
                        }
                    }
                    // If not confirmed, just close the dialog
                }
            }
            AppEvent::ConfirmationCancel => {
                state.confirmation_dialog = None;
            }
            AppEvent::AuthSetupNext => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.selected_method = match auth_state.selected_method {
                        AuthMethod::OAuth => AuthMethod::ApiKey,
                        AuthMethod::ApiKey => AuthMethod::Skip,
                        AuthMethod::Skip => AuthMethod::OAuth,
                    };
                }
            }
            AppEvent::AuthSetupPrevious => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.selected_method = match auth_state.selected_method {
                        AuthMethod::OAuth => AuthMethod::Skip,
                        AuthMethod::ApiKey => AuthMethod::OAuth,
                        AuthMethod::Skip => AuthMethod::ApiKey,
                    };
                }
            }
            AppEvent::AuthSetupSelect => {
                if let Some(ref auth_state) = state.auth_setup_state {
                    match auth_state.selected_method {
                        AuthMethod::OAuth => {
                            // Mark for async OAuth processing
                            state.pending_async_action = Some(AsyncAction::AuthSetupOAuth);
                        }
                        AuthMethod::ApiKey => {
                            if auth_state.api_key_input.is_empty() {
                                // Enter API key input mode
                                if let Some(ref mut auth_state) = state.auth_setup_state {
                                    auth_state.api_key_input = "sk-".to_string();
                                    auth_state.show_cursor = true;
                                }
                            } else {
                                // Save the API key
                                state.pending_async_action = Some(AsyncAction::AuthSetupApiKey);
                            }
                        }
                        AuthMethod::Skip => {
                            // Skip auth setup and go to main screen
                            state.auth_setup_state = None;
                            state.current_view = View::SessionList;
                            state.check_current_directory_status();
                            state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                        }
                    }
                }
            }
            AppEvent::AuthSetupCancel => {
                // Same as skip - go to main screen without auth
                state.auth_setup_state = None;
                state.current_view = View::SessionList;
                state.check_current_directory_status();
                state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
            }
            AppEvent::AuthSetupInputChar(ch) => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.api_key_input.push(ch);
                }
            }
            AppEvent::AuthSetupBackspace => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    if auth_state.api_key_input.is_empty() {
                        // Exit API key input mode
                        auth_state.show_cursor = false;
                    } else {
                        auth_state.api_key_input.pop();
                    }
                }
            }
            AppEvent::AuthSetupCheckStatus => {
                // Check if authentication was completed and transition if so
                if state.auth_setup_state.is_some() && !AppState::is_first_time_setup() {
                    // Authentication completed!
                    state.auth_setup_state = None;
                    state.current_view = View::SessionList;
                    state.check_current_directory_status();
                    state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                }
            }
            AppEvent::AuthSetupRefresh => {
                // Manual refresh - check authentication status immediately
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    if !AppState::is_first_time_setup() {
                        // Authentication completed!
                        state.auth_setup_state = None;
                        state.current_view = View::SessionList;
                        state.check_current_directory_status();
                        state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                    } else {
                        // Still waiting - update message
                        auth_state.error_message = Some("Still waiting for authentication. Complete the process in the terminal window.\n\nPress 'r' to refresh or 'Esc' to cancel.".to_string());
                    }
                }
            }
            AppEvent::AuthSetupShowCommand => {
                // Show alternative authentication methods
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.error_message = Some(
                        "📋 Alternative Authentication Methods:\n\n\
                         1. If the OAuth URL didn't appear, check the container logs\n\n\
                         2. Use API Key authentication instead (press Up/Down to switch)\n\n\
                         3. Run authentication manually in a terminal:\n\
                            docker exec -it agents-box-auth /bin/bash\n\
                            claude auth login\n\n\
                         Press 'Esc' to go back."
                            .to_string(),
                    );
                }
            }
            // File finder events
            AppEvent::FileFinderNavigateUp => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.file_finder.move_selection_up();
                }
            }
            AppEvent::FileFinderNavigateDown => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.file_finder.move_selection_down();
                }
            }
            AppEvent::FileFinderSelectFile => {
                if let Some(ref mut session_state) = state.new_session_state {
                    if let Some(selected_file) = session_state.file_finder.get_selected_file() {
                        // Replace @query with the selected file path
                        let file_path = &selected_file.relative_path;
                        let at_pos = session_state.file_finder.at_symbol_position;
                        let query_end_pos = at_pos + 1 + session_state.file_finder.query.len();

                        // Construct new prompt by replacing @query with file path
                        let current_text = session_state.boss_prompt.to_string();
                        let mut new_prompt =
                            String::with_capacity(current_text.len() + file_path.len());
                        new_prompt.push_str(&current_text[..at_pos]);
                        new_prompt.push_str(file_path);
                        if query_end_pos < current_text.len() {
                            new_prompt.push_str(&current_text[query_end_pos..]);
                        }

                        session_state.boss_prompt =
                            crate::app::state::TextEditor::from_string(&new_prompt);
                        session_state.file_finder.deactivate();
                    }
                }
            }
            AppEvent::FileFinderCancel => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.file_finder.deactivate();
                }
            }
            // Git view events
            AppEvent::ShowGitView => {
                tracing::info!("Showing git view");
                state.show_git_view();
                tracing::info!(
                    "Git view state after show: current_view = {:?}, git_state = {}",
                    state.current_view,
                    state.git_view_state.is_some()
                );
            }
            AppEvent::GitViewSwitchTab => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.switch_tab();
                }
            }
            AppEvent::GitViewNextFile => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.next_file();
                }
            }
            AppEvent::GitViewPrevFile => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.previous_file();
                }
            }
            AppEvent::GitViewScrollUp => {
                if let Some(ref mut git_state) = state.git_view_state {
                    match git_state.active_tab {
                        crate::components::git_view::GitTab::Diff => git_state.scroll_diff_up(),
                        crate::components::git_view::GitTab::Markdown => git_state.scroll_markdown_up(),
                        _ => {}
                    }
                }
            }
            AppEvent::GitViewScrollDown => {
                if let Some(ref mut git_state) = state.git_view_state {
                    match git_state.active_tab {
                        crate::components::git_view::GitTab::Diff => git_state.scroll_diff_down(),
                        crate::components::git_view::GitTab::Markdown => git_state.scroll_markdown_down(),
                        _ => {}
                    }
                }
            }
            AppEvent::GitViewToggleFolder => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.toggle_folder();
                }
            }
            AppEvent::GitViewExpandAll => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.expand_all_folders();
                }
            }
            AppEvent::GitViewCollapseAll => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.collapse_all_folders();
                }
            }
            AppEvent::GitViewCommitPush => {
                state.git_commit_and_push();
            }
            AppEvent::GitViewBack => {
                state.current_view = crate::app::state::View::SessionList;
                state.git_view_state = None;
            }
            // Commit message input events
            AppEvent::GitViewStartCommit => {
                tracing::info!("Processing GitViewStartCommit event");
                if let Some(ref mut git_state) = state.git_view_state {
                    tracing::info!("Git state found, starting commit message input");
                    git_state.start_commit_message_input();
                    state.add_info_notification(
                        "📝 Enter commit message and press Enter to commit & push".to_string(),
                    );
                } else {
                    tracing::warn!("No git state available for GitViewStartCommit");
                }
            }
            AppEvent::GitViewCommitInputChar(ch) => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.add_char_to_commit_message(ch);
                }
            }
            AppEvent::GitViewCommitBackspace => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.backspace_commit_message();
                }
            }
            AppEvent::GitViewCommitCursorLeft => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.move_commit_cursor_left();
                }
            }
            AppEvent::GitViewCommitCursorRight => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.move_commit_cursor_right();
                }
            }
            AppEvent::GitViewCommitCancel => {
                if let Some(ref mut git_state) = state.git_view_state {
                    git_state.cancel_commit_message_input();
                }
            }
            AppEvent::GitViewCommitConfirm => {
                state.git_commit_and_push();
            }
            AppEvent::GitCommitAndPush => {
                tracing::info!("Direct git commit and push from main view");
                state.git_commit_and_push();
            }
            AppEvent::QuickCommitStart => {
                tracing::info!("Starting quick commit dialog");
                state.start_quick_commit();
            }
            AppEvent::QuickCommitInputChar(ch) => {
                state.add_char_to_quick_commit(ch);
            }
            AppEvent::QuickCommitBackspace => {
                state.backspace_quick_commit();
            }
            AppEvent::QuickCommitCursorLeft => {
                state.move_quick_commit_cursor_left();
            }
            AppEvent::QuickCommitCursorRight => {
                state.move_quick_commit_cursor_right();
            }
            AppEvent::QuickCommitConfirm => {
                state.confirm_quick_commit();
            }
            AppEvent::QuickCommitCancel => {
                state.cancel_quick_commit();
            }
            AppEvent::GitCommitSuccess(message) => {
                tracing::info!("Git commit successful: {}", message);
                // Add success notification
                state.add_success_notification(format!("✅ {}", message));
                // Exit git view and return to main session list
                state.current_view = crate::app::state::View::SessionList;
                state.git_view_state = None;
                tracing::info!("Returned to session list after successful commit");
            }
            // Mouse events are handled directly in the main event loop
            AppEvent::MouseClick { .. } |
            AppEvent::MouseDragStart { .. } |
            AppEvent::MouseDragEnd { .. } |
            AppEvent::MouseDragging { .. } => {
                // These are processed by handle_mouse_event
            }
        }
    }
}
