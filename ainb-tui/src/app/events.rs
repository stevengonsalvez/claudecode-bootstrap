// ABOUTME: Event handling system for keyboard input and app actions

#![allow(dead_code)]

use crate::app::{
    AppState,
    state::{AsyncAction, AuthMethod, View},
};
use crate::credentials;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::info;

// Layout configuration - sessions pane width as percentage of terminal width
const SESSIONS_PANE_WIDTH_PERCENTAGE: f32 = 0.4;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    GoToHomeScreen,  // Return to home screen from any view
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
    OpenQuickShell,  // Open shell in selected workspace/session directory
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
    NewSessionBackspaceWord,  // Delete word backward (Shift+Backspace)
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
    // Agent selection events (new session flow)
    NewSessionAgentNext,
    NewSessionAgentPrev,
    NewSessionAgentSelect,
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
    // AINB 2.0: Home screen events
    HomeScreenSelectTile,        // Select current tile (Enter)
    HomeScreenNavigateUp,        // Navigate up in tile grid
    HomeScreenNavigateDown,      // Navigate down in tile grid
    HomeScreenNavigateLeft,      // Navigate left in tile grid
    HomeScreenNavigateRight,     // Navigate right in tile grid
    // AINB 2.0: Home screen V2 events (sidebar + card grid)
    HomeScreenToggleFocus,       // Toggle focus between sidebar and card grid (Tab)
    HomeScreenSidebarUp,         // Navigate up in sidebar
    HomeScreenSidebarDown,       // Navigate down in sidebar
    HomeScreenSidebarSelect,     // Select current sidebar item (Enter)
    GoToAgentSelection,          // Navigate to agent selection view
    GoToCatalog,                 // Navigate to catalog view (coming soon)
    GoToConfig,                  // Navigate to config view
    GoToSessionList,             // Navigate to session list view
    GoToStats,                   // Navigate to stats view (coming soon)
    // AINB 2.0: Agent selection events
    AgentSelectionBack,          // Return to home screen (Esc)
    AgentSelectionNextProvider,  // Navigate to next provider
    AgentSelectionPrevProvider,  // Navigate to previous provider
    AgentSelectionNextModel,     // Navigate to next model
    AgentSelectionPrevModel,     // Navigate to previous model
    AgentSelectionToggleExpand,  // Toggle provider expand
    AgentSelectionSelect,        // Select current agent (Enter)
    // AINB 2.0: Config screen events
    ConfigBack,                  // Return to home screen (Esc)
    ConfigNextCategory,          // Navigate to next category
    ConfigPrevCategory,          // Navigate to previous category
    ConfigNextSetting,           // Navigate to next setting
    ConfigPrevSetting,           // Navigate to previous setting
    ConfigSwitchPane,            // Switch between category and settings pane (Tab)
    ConfigEditSetting,           // Start editing current setting (Enter)
    ConfigSaveEdit,              // Save current edit (Enter while editing)
    ConfigCancelEdit,            // Cancel current edit (Esc while editing)
    ConfigEditChar(char),        // Input character while editing
    ConfigEditBackspace,         // Backspace while editing
    ConfigSaveAll,               // Save all settings (S)
    // API Key configuration
    ConfigApiKeyStart,           // Start API key input mode (when on API Key Status)
    ConfigApiKeySave,            // Save the entered API key to keychain
    ConfigApiKeyDelete,          // Delete stored API key
    // Auth provider popup
    AuthProviderPopupOpen,       // Open the auth provider popup
    AuthProviderPopupClose,      // Close the popup (Esc)
    AuthProviderPopupNext,       // Navigate to next provider
    AuthProviderPopupPrev,       // Navigate to previous provider
    AuthProviderPopupSelect,     // Select current provider (Enter)
    AuthProviderPopupInputChar(char), // Input character for API key
    AuthProviderPopupBackspace,  // Backspace in API key input
    AuthProviderPopupDeleteKey,  // Delete stored API key (D)
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
            tracing::debug!("Help is visible, handling key: {:?}", key_event.code);
            match key_event.code {
                KeyCode::Char('?' | 'H') | KeyCode::Esc => {
                    tracing::info!("Toggling help off via {:?}", key_event.code);
                    return Some(AppEvent::ToggleHelp);
                }
                _ => {
                    tracing::debug!("Ignoring key {:?} while help visible", key_event.code);
                    return None;
                }
            }
        }

        // Handle global help toggle first (should work from any view)
        // Supports both '?' and Shift+H
        if matches!(key_event.code, KeyCode::Char('?' | 'H')) {
            return Some(AppEvent::ToggleHelp);
        }

        // AINB 2.0: Handle home screen view
        if state.current_view == View::HomeScreen {
            return Self::handle_home_screen_keys(key_event, state);
        }

        // AINB 2.0: Handle agent selection view
        if state.current_view == View::AgentSelection {
            return Self::handle_agent_selection_keys(key_event, state);
        }

        // AINB 2.0: Handle auth provider popup (overlays config screen)
        if state.auth_provider_popup_state.show_popup {
            return Self::handle_auth_provider_popup_keys(key_event, state);
        }

        // AINB 2.0: Handle config screen view
        if state.current_view == View::Config {
            return Self::handle_config_screen_keys(key_event, state);
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
            // Return to home screen (quit only available from HomeScreen)
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::GoToHomeScreen),
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
            KeyCode::Char('$') => Some(AppEvent::OpenQuickShell), // Quick shell in current workspace/session

            // Tmux preview scroll mode (Shift + Up/Down)
            KeyCode::Up if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                Some(AppEvent::ScrollPreviewUp)
            }
            KeyCode::Down if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                Some(AppEvent::ScrollPreviewDown)
            }

            // Navigation keys depend on focused pane (arrow keys only)
            KeyCode::Down => {
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
            KeyCode::Up => {
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
            KeyCode::Left => {
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
            KeyCode::Right => {
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
                NewSessionStep::SelectAgent => match key_event.code {
                    KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                    KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::NewSessionAgentNext),
                    KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::NewSessionAgentPrev),
                    KeyCode::Enter => Some(AppEvent::NewSessionAgentSelect),
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
                        KeyCode::Backspace
                            if key_event.modifiers.contains(KeyModifiers::SHIFT) =>
                        {
                            Some(AppEvent::NewSessionBackspaceWord)
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
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::GoToHomeScreen),
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

    // AINB 2.0: Home screen key handling (V2 with sidebar and card grid)
    fn handle_home_screen_keys(key_event: KeyEvent, state: &AppState) -> Option<AppEvent> {
        use crate::components::home_screen_v2::HomeScreenFocus;

        tracing::debug!("HomeScreen V2 key handler: {:?}", key_event.code);

        // Global shortcuts that work regardless of focus (matches HomeTile shortcuts)
        match key_event.code {
            KeyCode::Char('a') => return Some(AppEvent::GoToAgentSelection),
            KeyCode::Char('c') => return Some(AppEvent::GoToCatalog),
            KeyCode::Char('C') => return Some(AppEvent::GoToConfig),
            KeyCode::Char('s') => return Some(AppEvent::GoToSessionList),
            KeyCode::Char('i') => return Some(AppEvent::GoToStats),
            KeyCode::Char('?') => return Some(AppEvent::ToggleHelp),
            KeyCode::Char('q') => return Some(AppEvent::Quit),
            KeyCode::Tab => return Some(AppEvent::HomeScreenToggleFocus),
            _ => {}
        }

        // Focus-specific navigation
        let focus = &state.home_screen_v2_state.focus;
        let event = match focus {
            HomeScreenFocus::Sidebar => {
                match key_event.code {
                    KeyCode::Up => Some(AppEvent::HomeScreenSidebarUp),
                    KeyCode::Down => Some(AppEvent::HomeScreenSidebarDown),
                    KeyCode::Enter => Some(AppEvent::HomeScreenSidebarSelect),
                    _ => None,
                }
            }
            HomeScreenFocus::CardGrid => {
                match key_event.code {
                    KeyCode::Up => Some(AppEvent::HomeScreenNavigateUp),
                    KeyCode::Down => Some(AppEvent::HomeScreenNavigateDown),
                    KeyCode::Left => Some(AppEvent::HomeScreenNavigateLeft),
                    KeyCode::Right => Some(AppEvent::HomeScreenNavigateRight),
                    KeyCode::Enter => Some(AppEvent::HomeScreenSelectTile),
                    _ => None,
                }
            }
        };

        tracing::debug!("HomeScreen V2 key handler returning: {:?}", event);
        event
    }

    // AINB 2.0: Agent selection key handling
    fn handle_agent_selection_keys(key_event: KeyEvent, state: &AppState) -> Option<AppEvent> {
        let agent_state = &state.agent_selection_state;

        // Check if a provider is expanded (showing models)
        if agent_state.expanded_provider.is_some() {
            match key_event.code {
                KeyCode::Esc => Some(AppEvent::AgentSelectionBack),
                KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::AgentSelectionPrevModel),
                KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::AgentSelectionNextModel),
                KeyCode::Tab => Some(AppEvent::AgentSelectionNextProvider),
                KeyCode::BackTab => Some(AppEvent::AgentSelectionPrevProvider),
                KeyCode::Enter => Some(AppEvent::AgentSelectionSelect),
                KeyCode::Char(' ') => Some(AppEvent::AgentSelectionToggleExpand),
                _ => None,
            }
        } else {
            match key_event.code {
                KeyCode::Esc => Some(AppEvent::AgentSelectionBack),
                KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::AgentSelectionPrevProvider),
                KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::AgentSelectionNextProvider),
                KeyCode::Enter | KeyCode::Char(' ') => Some(AppEvent::AgentSelectionToggleExpand),
                _ => None,
            }
        }
    }

    fn handle_config_screen_keys(key_event: KeyEvent, state: &AppState) -> Option<AppEvent> {
        let config_state = &state.config_screen_state;
        tracing::debug!(
            "Config screen key handler: {:?}, editing: {}, api_key_mode: {}",
            key_event.code,
            config_state.editing,
            config_state.api_key_input_mode
        );

        // API key input mode - special handling (saves to keychain)
        if config_state.api_key_input_mode {
            match key_event.code {
                KeyCode::Enter => Some(AppEvent::ConfigApiKeySave),
                KeyCode::Esc => Some(AppEvent::ConfigCancelEdit),
                KeyCode::Backspace => Some(AppEvent::ConfigEditBackspace),
                KeyCode::Char(c) => Some(AppEvent::ConfigEditChar(c)),
                _ => None,
            }
        } else if config_state.editing {
            // Normal editing mode - handle text input
            match key_event.code {
                KeyCode::Enter => Some(AppEvent::ConfigSaveEdit),
                KeyCode::Esc => Some(AppEvent::ConfigCancelEdit),
                KeyCode::Backspace => Some(AppEvent::ConfigEditBackspace),
                KeyCode::Char(c) => Some(AppEvent::ConfigEditChar(c)),
                _ => None,
            }
        } else {
            // Navigation mode - check if we're on auth settings
            let is_auth_category = config_state.selected_category == 0;  // Authentication category
            let on_claude_auth = is_auth_category && config_state.selected_setting == 0;  // Claude Authentication

            match key_event.code {
                KeyCode::Esc => Some(AppEvent::ConfigBack),
                KeyCode::Tab => Some(AppEvent::ConfigSwitchPane),
                KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::ConfigPrevSetting),
                KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::ConfigNextSetting),
                KeyCode::Left | KeyCode::Char('h') => Some(AppEvent::ConfigPrevCategory),
                KeyCode::Right | KeyCode::Char('l') => Some(AppEvent::ConfigNextCategory),
                KeyCode::Enter => {
                    if on_claude_auth {
                        // Open the auth provider popup
                        Some(AppEvent::AuthProviderPopupOpen)
                    } else {
                        Some(AppEvent::ConfigEditSetting)
                    }
                }
                KeyCode::Char('s' | 'S') => Some(AppEvent::ConfigSaveAll),
                _ => None,
            }
        }
    }

    // AINB 2.0: Auth provider popup key handling
    fn handle_auth_provider_popup_keys(key_event: KeyEvent, state: &AppState) -> Option<AppEvent> {
        let popup_state = &state.auth_provider_popup_state;

        if popup_state.is_entering_key {
            // API key input mode
            match key_event.code {
                KeyCode::Enter => Some(AppEvent::AuthProviderPopupSelect),
                KeyCode::Esc => Some(AppEvent::AuthProviderPopupClose),
                KeyCode::Backspace => Some(AppEvent::AuthProviderPopupBackspace),
                KeyCode::Char(c) => Some(AppEvent::AuthProviderPopupInputChar(c)),
                _ => None,
            }
        } else {
            // Navigation mode
            match key_event.code {
                KeyCode::Esc => Some(AppEvent::AuthProviderPopupClose),
                KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::AuthProviderPopupPrev),
                KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::AuthProviderPopupNext),
                KeyCode::Enter => Some(AppEvent::AuthProviderPopupSelect),
                KeyCode::Char('d' | 'D') => Some(AppEvent::AuthProviderPopupDeleteKey),
                _ => None,
            }
        }
    }

    pub fn process_event(event: AppEvent, state: &mut AppState) {
        match event {
            AppEvent::Quit => state.quit(),
            AppEvent::GoToHomeScreen => {
                tracing::info!("Navigating to HomeScreen");
                state.current_view = View::HomeScreen;
            }
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
            AppEvent::NewSessionBackspaceWord => {
                tracing::debug!("Event: NewSessionBackspaceWord");
                state.new_session_backspace_word();
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
            // Agent selection events (new session flow)
            AppEvent::NewSessionAgentNext => {
                tracing::debug!("Event: NewSessionAgentNext");
                state.new_session_next_agent();
            }
            AppEvent::NewSessionAgentPrev => {
                tracing::debug!("Event: NewSessionAgentPrev");
                state.new_session_prev_agent();
            }
            AppEvent::NewSessionAgentSelect => {
                tracing::info!("Event: NewSessionAgentSelect");
                let shell_selected = state.new_session_select_agent();
                if shell_selected {
                    // Shell was selected - find or create workspace and open shell
                    if let Some(ref session_state) = state.new_session_state {
                        if let Some(repo_path) = session_state.get_selected_repo_path() {
                            tracing::info!("Opening shell in workspace: {:?}", repo_path);

                            // Find the workspace index for this repo
                            let workspace_idx = state.workspaces.iter()
                                .position(|w| w.path == repo_path);

                            if let Some(idx) = workspace_idx {
                                state.pending_async_action = Some(
                                    AsyncAction::OpenWorkspaceShell {
                                        workspace_index: idx,
                                        target_dir: None, // Open in workspace root
                                    }
                                );
                            } else {
                                state.add_warning_notification("Workspace not found".to_string());
                            }
                        }
                    }
                    state.new_session_state = None;
                    state.current_view = crate::app::state::View::SessionList;
                }
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
                    "[ACTION] State: workspace_idx={:?}, session_idx={:?}, shell_selected={}, is_other_tmux={}, other_tmux_idx={:?}",
                    state.selected_workspace_index,
                    state.selected_session_index,
                    state.shell_selected,
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
                } else if state.shell_selected {
                    // Shell session selected - attach to its tmux session
                    if let Some(workspace_idx) = state.selected_workspace_index {
                        if let Some(workspace) = state.workspaces.get(workspace_idx) {
                            if let Some(shell) = &workspace.shell_session {
                                let session_name = shell.tmux_session_name.clone();
                                tracing::info!("[ACTION] Attaching to workspace shell: {}", session_name);
                                state.pending_async_action = Some(AsyncAction::AttachToOtherTmux(session_name));
                            } else {
                                tracing::warn!("[ACTION] Shell selected but no shell session found in workspace");
                                state.add_error_notification("No shell session found".to_string());
                            }
                        }
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
                // Clear attached session and return to home screen
                state.attached_session_id = None;
                state.current_view = View::HomeScreen;
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
                } else if state.shell_selected {
                    // Shell session selected - show kill shell confirmation
                    if let Some(workspace_idx) = state.selected_workspace_index {
                        if state.workspaces.get(workspace_idx).and_then(|w| w.shell_session.as_ref()).is_some() {
                            state.show_kill_shell_confirmation(workspace_idx);
                        }
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
            AppEvent::OpenQuickShell => {
                // Open workspace shell and optionally cd to session's worktree
                if let Some(workspace_idx) = state.selected_workspace_index {
                    // Get target directory - session worktree if selected, otherwise workspace root
                    let target_dir = if let Some(session) = state.selected_session() {
                        // Session selected - cd to its worktree
                        Some(std::path::PathBuf::from(&session.workspace_path))
                    } else {
                        // Just workspace selected - cd to workspace root (or None to stay where we are)
                        None
                    };

                    tracing::info!("Opening workspace shell, target_dir: {:?}", target_dir);
                    state.pending_async_action = Some(AsyncAction::OpenWorkspaceShell {
                        workspace_index: workspace_idx,
                        target_dir,
                    });
                } else {
                    state.add_warning_notification("No workspace selected".to_string());
                }
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
                            crate::app::state::ConfirmAction::KillWorkspaceShell(workspace_idx) => {
                                state.pending_async_action =
                                    Some(AsyncAction::KillWorkspaceShell(workspace_idx));
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
                            // Skip auth setup and go to home screen
                            state.auth_setup_state = None;
                            state.current_view = View::HomeScreen;
                            state.check_current_directory_status();
                            state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                        }
                    }
                }
            }
            AppEvent::AuthSetupCancel => {
                // Same as skip - go to home screen without auth
                state.auth_setup_state = None;
                state.current_view = View::HomeScreen;
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
                    state.current_view = View::HomeScreen;
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
                        state.current_view = View::HomeScreen;
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
                state.current_view = crate::app::state::View::HomeScreen;
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
                // Exit git view and return to home screen
                state.current_view = crate::app::state::View::HomeScreen;
                state.git_view_state = None;
                tracing::info!("Returned to home screen after successful commit");
            }
            // AINB 2.0: Home screen events
            AppEvent::HomeScreenSelectTile => {
                use crate::app::state::HomeTile;
                tracing::info!("HomeScreenSelectTile event - processing tile selection");
                if let Some(tile) = state.home_screen_state.selected().cloned() {
                    tracing::info!("Selected tile: {:?}", tile);
                    match tile {
                        HomeTile::Agents => {
                            tracing::info!("Navigating to AgentSelection view");
                            state.current_view = View::AgentSelection;
                        }
                        HomeTile::Sessions => {
                            tracing::info!("Navigating to SessionList view");
                            state.current_view = View::SessionList;
                        }
                        HomeTile::Help => {
                            tracing::info!("Toggling help overlay visible");
                            state.help_visible = true;
                        }
                        HomeTile::Config => {
                            tracing::info!("Navigating to Config view");
                            state.current_view = View::Config;
                        }
                        HomeTile::Catalog | HomeTile::Stats => {
                            tracing::info!("Tile {:?} - Coming Soon", tile);
                            // Coming soon - show notification
                            state.add_info_notification(format!(
                                "{} {} - Coming Soon!",
                                tile.icon(),
                                tile.label()
                            ));
                        }
                    }
                } else {
                    tracing::warn!("No tile selected in HomeScreenState");
                }
            }
            AppEvent::HomeScreenNavigateUp => {
                tracing::debug!("HomeScreen navigate up");
                state.home_screen_state.select_up();
                // Also update card grid for V2
                state.home_screen_v2_state.card_grid.move_up();
            }
            AppEvent::HomeScreenNavigateDown => {
                tracing::debug!("HomeScreen navigate down");
                state.home_screen_state.select_down();
                // Also update card grid for V2
                state.home_screen_v2_state.card_grid.move_down();
            }
            AppEvent::HomeScreenNavigateLeft => {
                tracing::debug!("HomeScreen navigate left");
                state.home_screen_state.select_left();
                // Also update card grid for V2
                state.home_screen_v2_state.card_grid.move_left();
            }
            AppEvent::HomeScreenNavigateRight => {
                tracing::debug!("HomeScreen navigate right");
                state.home_screen_state.select_right();
                // Also update card grid for V2
                state.home_screen_v2_state.card_grid.move_right();
            }
            // AINB 2.0: Home screen V2 events
            AppEvent::HomeScreenToggleFocus => {
                tracing::debug!("HomeScreen V2 toggle focus");
                state.home_screen_v2_state.toggle_focus();
            }
            AppEvent::HomeScreenSidebarUp => {
                tracing::debug!("HomeScreen V2 sidebar up");
                state.home_screen_v2_state.sidebar.move_up();
            }
            AppEvent::HomeScreenSidebarDown => {
                tracing::debug!("HomeScreen V2 sidebar down");
                state.home_screen_v2_state.sidebar.move_down();
            }
            AppEvent::HomeScreenSidebarSelect => {
                use crate::components::sidebar::SidebarItem;
                tracing::debug!("HomeScreen V2 sidebar select");
                let selected = state.home_screen_v2_state.sidebar.selected_item();
                match selected {
                    SidebarItem::Agents => {
                        state.current_view = View::AgentSelection;
                    }
                    SidebarItem::Catalog => {
                        state.add_info_notification("Skill catalog coming soon!".to_string());
                    }
                    SidebarItem::Config => {
                        state.current_view = View::Config;
                    }
                    SidebarItem::Sessions => {
                        state.current_view = View::SessionList;
                    }
                    SidebarItem::Stats => {
                        state.add_info_notification("Usage & Analytics coming soon!".to_string());
                    }
                    SidebarItem::Help => {
                        state.help_visible = true;
                    }
                }
            }
            AppEvent::GoToAgentSelection => {
                tracing::info!("Navigating to AgentSelection");
                state.current_view = View::AgentSelection;
            }
            AppEvent::GoToCatalog => {
                state.add_info_notification("Skill catalog coming soon!".to_string());
            }
            AppEvent::GoToConfig => {
                tracing::info!("Navigating to Config");
                state.current_view = View::Config;
            }
            AppEvent::GoToSessionList => {
                tracing::info!("Navigating to SessionList");
                state.current_view = View::SessionList;
            }
            AppEvent::GoToStats => {
                state.add_info_notification("Usage & Analytics coming soon!".to_string());
            }
            // AINB 2.0: Agent selection events
            AppEvent::AgentSelectionBack => {
                state.current_view = View::HomeScreen;
            }
            AppEvent::AgentSelectionNextProvider => {
                state.agent_selection_state.select_next_provider();
            }
            AppEvent::AgentSelectionPrevProvider => {
                state.agent_selection_state.select_prev_provider();
            }
            AppEvent::AgentSelectionNextModel => {
                state.agent_selection_state.select_next_model();
            }
            AppEvent::AgentSelectionPrevModel => {
                state.agent_selection_state.select_prev_model();
            }
            AppEvent::AgentSelectionToggleExpand => {
                state.agent_selection_state.toggle_expand();
            }
            AppEvent::AgentSelectionSelect => {
                if state.agent_selection_state.is_current_available() {
                    // Store selected agent and proceed to session creation
                    state.add_success_notification(format!(
                        "Selected: {} - {}",
                        state.agent_selection_state.current_provider().map(|p| p.name.as_str()).unwrap_or("Unknown"),
                        state.agent_selection_state.current_model().map(|m| m.name.as_str()).unwrap_or("Unknown")
                    ));
                    // Go to session list or new session
                    state.current_view = View::SessionList;
                } else {
                    state.add_warning_notification("This agent is not available yet.".to_string());
                }
            }
            // AINB 2.0: Config screen events
            AppEvent::ConfigBack => {
                tracing::info!("Navigating back from Config to HomeScreen");
                state.current_view = View::HomeScreen;
            }
            AppEvent::ConfigNextCategory => {
                let num_categories = state.config_screen_state.categories.len();
                if num_categories > 0 {
                    state.config_screen_state.selected_category =
                        (state.config_screen_state.selected_category + 1) % num_categories;
                    state.config_screen_state.selected_setting = 0;
                }
            }
            AppEvent::ConfigPrevCategory => {
                let num_categories = state.config_screen_state.categories.len();
                if num_categories > 0 {
                    state.config_screen_state.selected_category =
                        state.config_screen_state.selected_category
                            .checked_sub(1)
                            .unwrap_or(num_categories - 1);
                    state.config_screen_state.selected_setting = 0;
                }
            }
            AppEvent::ConfigNextSetting => {
                let current_category = &state.config_screen_state.categories[state.config_screen_state.selected_category];
                if let Some(settings) = state.config_screen_state.settings.get(current_category) {
                    if !settings.is_empty() {
                        state.config_screen_state.selected_setting =
                            (state.config_screen_state.selected_setting + 1) % settings.len();
                    }
                }
            }
            AppEvent::ConfigPrevSetting => {
                let current_category = &state.config_screen_state.categories[state.config_screen_state.selected_category];
                if let Some(settings) = state.config_screen_state.settings.get(current_category) {
                    if !settings.is_empty() {
                        state.config_screen_state.selected_setting =
                            state.config_screen_state.selected_setting
                                .checked_sub(1)
                                .unwrap_or(settings.len() - 1);
                    }
                }
            }
            AppEvent::ConfigSwitchPane => {
                // Toggle focus between categories and settings - for now just toggle category/setting focus
                tracing::debug!("Config switch pane - toggling focus");
            }
            AppEvent::ConfigEditSetting => {
                let current_category = state.config_screen_state.categories[state.config_screen_state.selected_category];
                if let Some(settings) = state.config_screen_state.settings.get(&current_category) {
                    if let Some(setting) = settings.get(state.config_screen_state.selected_setting) {
                        state.config_screen_state.editing = true;
                        state.config_screen_state.edit_buffer = setting.value.display();
                        tracing::info!("Started editing setting: {}", setting.label);
                    }
                }
            }
            AppEvent::ConfigSaveEdit => {
                let current_category = state.config_screen_state.categories[state.config_screen_state.selected_category];
                if let Some(settings) = state.config_screen_state.settings.get_mut(&current_category) {
                    if let Some(setting) = settings.get_mut(state.config_screen_state.selected_setting) {
                        let new_value = state.config_screen_state.edit_buffer.clone();
                        // Update the value based on the type
                        setting.value = match &setting.value {
                            crate::app::state::ConfigValue::Text(_) => crate::app::state::ConfigValue::Text(new_value),
                            crate::app::state::ConfigValue::Secret(_) => crate::app::state::ConfigValue::Secret(new_value),
                            crate::app::state::ConfigValue::Bool(_) => crate::app::state::ConfigValue::Bool(new_value.to_lowercase() == "true"),
                            crate::app::state::ConfigValue::Number(_) => crate::app::state::ConfigValue::Number(new_value.parse().unwrap_or(0)),
                            crate::app::state::ConfigValue::Choice(options, _) => {
                                // Try to find the index of the entered value
                                let idx = options.iter().position(|o| o == &new_value).unwrap_or(0);
                                crate::app::state::ConfigValue::Choice(options.clone(), idx)
                            }
                        };
                        tracing::info!("Saved setting: {} = {}", setting.label, setting.value.display());
                    }
                }
                state.config_screen_state.editing = false;
                state.config_screen_state.edit_buffer.clear();
            }
            AppEvent::ConfigCancelEdit => {
                state.config_screen_state.editing = false;
                state.config_screen_state.edit_buffer.clear();
                tracing::info!("Cancelled editing");
            }
            AppEvent::ConfigEditChar(c) => {
                state.config_screen_state.edit_buffer.push(c);
            }
            AppEvent::ConfigEditBackspace => {
                state.config_screen_state.edit_buffer.pop();
            }
            AppEvent::ConfigSaveAll => {
                tracing::info!("Saving all settings to config file");

                // Apply ConfigScreenState settings to AppConfig
                state.config_screen_state.apply_to_app_config(&mut state.app_config);

                // Save to disk
                match state.app_config.save() {
                    Ok(()) => {
                        state.add_success_notification("Settings saved to config.toml".to_string());
                        tracing::info!("Settings saved to ~/.agents-in-a-box/config/config.toml");
                    }
                    Err(e) => {
                        state.add_error_notification(format!("Failed to save settings: {}", e));
                        tracing::error!("Failed to save config: {}", e);
                    }
                }
            }
            // API Key configuration events
            AppEvent::ConfigApiKeyStart => {
                tracing::info!("Starting API key input mode");
                state.config_screen_state.api_key_input_mode = true;
                state.config_screen_state.edit_buffer.clear();
                state.add_info_notification("Enter your Anthropic API key (starts with sk-ant-)".to_string());
            }
            AppEvent::ConfigApiKeySave => {
                let api_key = state.config_screen_state.edit_buffer.clone();
                tracing::info!("Saving API key to keychain");

                match credentials::store_anthropic_api_key(&api_key) {
                    Ok(()) => {
                        state.add_success_notification("API key saved to system keychain".to_string());
                        tracing::info!("API key successfully stored in keychain");

                        // Update auth status to show API key configured
                        let masked = credentials::get_anthropic_api_key_masked();
                        let status = format!("API Key ({})", masked);
                        let auth_category = crate::app::state::ConfigCategory::Authentication;
                        if let Some(settings) = state.config_screen_state.settings.get_mut(&auth_category) {
                            if let Some(status_setting) = settings.iter_mut().find(|s| s.key == "claude_auth") {
                                status_setting.value = crate::app::state::ConfigValue::Text(status);
                            }
                        }
                    }
                    Err(e) => {
                        state.add_error_notification(format!("Failed to save API key: {}", e));
                        tracing::error!("Failed to store API key: {}", e);
                    }
                }

                state.config_screen_state.api_key_input_mode = false;
                state.config_screen_state.edit_buffer.clear();
            }
            AppEvent::ConfigApiKeyDelete => {
                tracing::info!("Deleting API key from keychain");

                match credentials::delete_anthropic_api_key() {
                    Ok(()) => {
                        state.add_success_notification("API key removed from system keychain".to_string());
                        tracing::info!("API key successfully deleted from keychain");

                        // Update auth status to show system auth
                        let auth_category = crate::app::state::ConfigCategory::Authentication;
                        if let Some(settings) = state.config_screen_state.settings.get_mut(&auth_category) {
                            if let Some(status_setting) = settings.iter_mut().find(|s| s.key == "claude_auth") {
                                status_setting.value = crate::app::state::ConfigValue::Text(
                                    "System Auth (Pro/Max Plan)".to_string()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        state.add_error_notification(format!("Failed to delete API key: {}", e));
                        tracing::error!("Failed to delete API key: {}", e);
                    }
                }
            }
            // Auth provider popup events
            AppEvent::AuthProviderPopupOpen => {
                tracing::info!("Opening auth provider popup");
                state.auth_provider_popup_state.show_popup = true;
                state.auth_provider_popup_state.refresh_providers();
            }
            AppEvent::AuthProviderPopupClose => {
                tracing::info!("Closing auth provider popup");
                state.auth_provider_popup_state.show_popup = false;
                state.auth_provider_popup_state.is_entering_key = false;
                state.auth_provider_popup_state.api_key_input.clear();
            }
            AppEvent::AuthProviderPopupNext => {
                state.auth_provider_popup_state.select_next();
            }
            AppEvent::AuthProviderPopupPrev => {
                state.auth_provider_popup_state.select_prev();
            }
            AppEvent::AuthProviderPopupSelect => {
                let popup_state = &state.auth_provider_popup_state;

                if popup_state.is_entering_key {
                    // Save the API key
                    let api_key = popup_state.api_key_input.clone();
                    tracing::info!("Saving API key from popup");

                    match credentials::store_anthropic_api_key(&api_key) {
                        Ok(()) => {
                            state.add_success_notification("API key saved to system keychain".to_string());

                            // Update config screen status
                            let masked = credentials::get_anthropic_api_key_masked();
                            let status = format!("API Key ({})", masked);
                            let auth_category = crate::app::state::ConfigCategory::Authentication;
                            if let Some(settings) = state.config_screen_state.settings.get_mut(&auth_category) {
                                if let Some(status_setting) = settings.iter_mut().find(|s| s.key == "claude_auth") {
                                    status_setting.value = crate::app::state::ConfigValue::Text(status);
                                }
                            }

                            // Persist auth provider to config.toml
                            state.app_config.authentication.claude_provider = crate::config::ClaudeAuthProvider::ApiKey;
                            if let Err(e) = state.app_config.save() {
                                tracing::warn!("Failed to save config: {}", e);
                            }

                            // Close popup and refresh
                            state.auth_provider_popup_state.show_popup = false;
                            state.auth_provider_popup_state.is_entering_key = false;
                            state.auth_provider_popup_state.api_key_input.clear();
                            state.auth_provider_popup_state.refresh_providers();
                        }
                        Err(e) => {
                            state.add_error_notification(format!("Failed to save API key: {}", e));
                        }
                    }
                } else {
                    // Check what's selected
                    if let Some(provider) = popup_state.current_provider() {
                        if !provider.available {
                            state.add_info_notification(format!("{} - Coming Soon!", provider.name));
                        } else if provider.id == "api_key" {
                            // Start API key input mode
                            state.auth_provider_popup_state.start_key_input();
                        } else if provider.id == "system" {
                            // System auth - just close and confirm
                            state.add_success_notification("Using system authentication (Pro/Max plan)".to_string());

                            // Delete any stored API key to switch to system auth
                            let _ = credentials::delete_anthropic_api_key();

                            // Update config screen status
                            let auth_category = crate::app::state::ConfigCategory::Authentication;
                            if let Some(settings) = state.config_screen_state.settings.get_mut(&auth_category) {
                                if let Some(status_setting) = settings.iter_mut().find(|s| s.key == "claude_auth") {
                                    status_setting.value = crate::app::state::ConfigValue::Text(
                                        "System Auth (Pro/Max Plan)".to_string()
                                    );
                                }
                            }

                            // Persist auth provider to config.toml
                            state.app_config.authentication.claude_provider = crate::config::ClaudeAuthProvider::SystemAuth;
                            if let Err(e) = state.app_config.save() {
                                tracing::warn!("Failed to save config: {}", e);
                            }

                            state.auth_provider_popup_state.show_popup = false;
                            state.auth_provider_popup_state.refresh_providers();
                        }
                    }
                }
            }
            AppEvent::AuthProviderPopupInputChar(c) => {
                state.auth_provider_popup_state.api_key_input.push(c);
            }
            AppEvent::AuthProviderPopupBackspace => {
                if state.auth_provider_popup_state.api_key_input.is_empty() {
                    // Exit key input mode
                    state.auth_provider_popup_state.cancel_key_input();
                } else {
                    state.auth_provider_popup_state.api_key_input.pop();
                }
            }
            AppEvent::AuthProviderPopupDeleteKey => {
                tracing::info!("Deleting API key from popup");
                match credentials::delete_anthropic_api_key() {
                    Ok(()) => {
                        state.add_success_notification("API key removed".to_string());
                        state.auth_provider_popup_state.refresh_providers();

                        // Update config screen
                        let auth_category = crate::app::state::ConfigCategory::Authentication;
                        if let Some(settings) = state.config_screen_state.settings.get_mut(&auth_category) {
                            if let Some(status_setting) = settings.iter_mut().find(|s| s.key == "claude_auth") {
                                status_setting.value = crate::app::state::ConfigValue::Text(
                                    "System Auth (Pro/Max Plan)".to_string()
                                );
                            }
                        }

                        // Persist switch to system auth in config.toml
                        state.app_config.authentication.claude_provider = crate::config::ClaudeAuthProvider::SystemAuth;
                        if let Err(e) = state.app_config.save() {
                            tracing::warn!("Failed to save config: {}", e);
                        }
                    }
                    Err(e) => {
                        state.add_error_notification(format!("Failed to delete: {}", e));
                    }
                }
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
