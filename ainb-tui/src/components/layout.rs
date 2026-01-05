// ABOUTME: Main layout component handling split-pane arrangement and bottom menu bar

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph},
};

// Premium color palette (TUI Style Guide)
const CORNFLOWER_BLUE: Color = Color::Rgb(100, 149, 237);
const GOLD: Color = Color::Rgb(255, 215, 0);
const SELECTION_GREEN: Color = Color::Rgb(100, 200, 100);
const WARNING_ORANGE: Color = Color::Rgb(255, 165, 0);
const DARK_BG: Color = Color::Rgb(25, 25, 35);
const PANEL_BG: Color = Color::Rgb(30, 30, 40);
const SOFT_WHITE: Color = Color::Rgb(220, 220, 230);
const MUTED_GRAY: Color = Color::Rgb(120, 120, 140);
const SUBDUED_BORDER: Color = Color::Rgb(60, 60, 80);

use super::{
    AgentSelectionComponent, AttachedTerminalComponent, AuthProviderPopupComponent, AuthSetupComponent, ClaudeChatComponent,
    ConfigScreenComponent, ConfirmationDialogComponent, HelpComponent, HomeScreenComponent,
    LiveLogsStreamComponent, LogsViewerComponent, NewSessionComponent, SessionListComponent,
    TmuxPreviewPane,
};
use crate::app::{AppState, state::View};

pub struct LayoutComponent {
    session_list: SessionListComponent,
    logs_viewer: LogsViewerComponent,
    claude_chat: ClaudeChatComponent,
    live_logs_stream: LiveLogsStreamComponent,
    help: HelpComponent,
    new_session: NewSessionComponent,
    confirmation_dialog: ConfirmationDialogComponent,
    attached_terminal: AttachedTerminalComponent,
    auth_setup: AuthSetupComponent,
    tmux_preview: TmuxPreviewPane,
    // AINB 2.0 components
    home_screen: HomeScreenComponent,
    agent_selection: AgentSelectionComponent,
    config_screen: ConfigScreenComponent,
    auth_provider_popup: AuthProviderPopupComponent,
}

impl LayoutComponent {
    pub fn new() -> Self {
        Self {
            session_list: SessionListComponent::new(),
            logs_viewer: LogsViewerComponent::new(),
            claude_chat: ClaudeChatComponent::new(),
            live_logs_stream: LiveLogsStreamComponent::new(),
            help: HelpComponent::new(),
            new_session: NewSessionComponent::new(),
            confirmation_dialog: ConfirmationDialogComponent::new(),
            attached_terminal: AttachedTerminalComponent::new(),
            auth_setup: AuthSetupComponent::new(),
            tmux_preview: TmuxPreviewPane::new(),
            // AINB 2.0 components
            home_screen: HomeScreenComponent::new(),
            agent_selection: AgentSelectionComponent::new(),
            config_screen: ConfigScreenComponent::new(),
            auth_provider_popup: AuthProviderPopupComponent::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, state: &AppState) {
        // Special handling for auth setup view (full screen)
        if state.current_view == View::AuthSetup {
            let centered_area = centered_rect(60, 60, frame.size());
            self.auth_setup.render(frame, centered_area, state);
            return;
        }

        // Special handling for attached terminal view (full screen)
        if state.current_view == View::AttachedTerminal {
            self.attached_terminal.render(frame, frame.size(), state);
            return;
        }

        // Special handling for git view (full screen)
        if state.current_view == View::GitView {
            if let Some(ref git_state) = state.git_view_state {
                crate::components::GitViewComponent::render(frame, frame.size(), git_state);
            }
            return;
        }

        // AINB 2.0: Home screen (full screen)
        if state.current_view == View::HomeScreen {
            tracing::debug!("Rendering HomeScreen view");
            self.home_screen.render(frame, frame.size(), state);
            // Render help overlay on top if visible
            if state.help_visible {
                tracing::debug!("Rendering help overlay on HomeScreen");
                self.help.render(frame, frame.size());
            }
            return;
        }

        // AINB 2.0: Agent selection (full screen)
        if state.current_view == View::AgentSelection {
            tracing::debug!("Rendering AgentSelection view");
            self.agent_selection.render(frame, frame.size(), state);
            // Render help overlay on top if visible
            if state.help_visible {
                tracing::debug!("Rendering help overlay on AgentSelection");
                self.help.render(frame, frame.size());
            }
            return;
        }

        // AINB 2.0: Config screen (full screen)
        if state.current_view == View::Config {
            tracing::debug!("Rendering Config view");
            self.config_screen.render(frame, frame.size(), state);

            // Render auth provider popup on top if visible
            if state.auth_provider_popup_state.show_popup {
                tracing::debug!("Rendering auth provider popup");
                self.auth_provider_popup.render(frame, frame.size(), state);
            }

            // Render help overlay on top if visible
            if state.help_visible {
                tracing::debug!("Rendering help overlay on Config");
                self.help.render(frame, frame.size());
            }
            return;
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Top status bar
                Constraint::Min(0),    // Main content area
                Constraint::Length(3), // Session info (single line + borders)
                Constraint::Length(3), // Bottom menu bar
            ])
            .split(frame.size());

        // Render top status bar
        self.render_status_bar(frame, main_layout[0], state);

        // Simple 2-panel layout: session list | logs (Claude chat is now a popup)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Session list
                Constraint::Percentage(60), // Live logs stream
            ])
            .split(main_layout[1]);

        // Pass focus information to components
        self.session_list.render(frame, content_chunks[0], state);

        // Render tmux preview if selected session has tmux, otherwise show live logs
        let selected_has_tmux = state
            .get_selected_session()
            .and_then(|s| s.tmux_session_name.as_ref())
            .is_some();

        if selected_has_tmux {
            // Render tmux preview pane
            self.tmux_preview.render(frame, content_chunks[1], state);
        } else {
            // Render traditional live logs stream
            self.live_logs_stream.render(frame, content_chunks[1], state);
        }

        // Render bottom logs area (traditional logs viewer)
        self.logs_viewer.render(frame, main_layout[2], state);

        // Render bottom menu bar
        self.render_menu_bar(frame, main_layout[3]);

        // Render help overlay if visible
        if state.help_visible {
            self.help.render(frame, frame.size());
        }

        // Render new session overlay if visible
        if state.current_view == View::NewSession || state.current_view == View::SearchWorkspace {
            self.new_session.render(frame, frame.size(), state);
        }

        // Render Claude chat popup if visible
        if state.current_view == View::ClaudeChat {
            let popup_area = centered_rect(80, 80, frame.size());
            self.claude_chat.render(frame, popup_area, state);
        }

        // Render confirmation dialog if visible (highest priority overlay)
        if state.confirmation_dialog.is_some() {
            self.confirmation_dialog.render(frame, frame.size(), state);
        }

        // Render quick commit dialog if visible
        if state.is_in_quick_commit_mode() {
            self.render_quick_commit_dialog(frame, frame.size(), state);
        }

        // Render notifications (top-right corner)
        self.render_notifications(frame, frame.size(), state);
    }

    /// Get mutable reference to live logs component for scroll handling
    pub fn live_logs_mut(&mut self) -> &mut LiveLogsStreamComponent {
        &mut self.live_logs_stream
    }

    /// Get mutable reference to tmux preview component for scroll handling
    pub fn tmux_preview_mut(&mut self) -> &mut TmuxPreviewPane {
        &mut self.tmux_preview
    }

    fn render_menu_bar(&self, frame: &mut Frame, area: Rect) {
        // Premium styled command bar with separators
        let menu_spans = vec![
            // Navigation group
            Span::styled("n", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
            Span::styled("ew ", Style::default().fg(MUTED_GRAY)),
            Span::styled("s", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
            Span::styled("earch ", Style::default().fg(MUTED_GRAY)),
            Span::styled("E", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
            Span::styled("xpand ", Style::default().fg(MUTED_GRAY)),
            Span::styled("Tab", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
            Span::styled(" focus", Style::default().fg(MUTED_GRAY)),
            Span::styled(" │ ", Style::default().fg(SUBDUED_BORDER)),
            // Actions group
            Span::styled("a", Style::default().fg(SELECTION_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("ttach ", Style::default().fg(MUTED_GRAY)),
            Span::styled("e", Style::default().fg(SELECTION_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" restart ", Style::default().fg(MUTED_GRAY)),
            Span::styled("d", Style::default().fg(Color::Rgb(230, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::styled("elete", Style::default().fg(MUTED_GRAY)),
            Span::styled(" │ ", Style::default().fg(SUBDUED_BORDER)),
            // Git group
            Span::styled("g", Style::default().fg(CORNFLOWER_BLUE).add_modifier(Modifier::BOLD)),
            Span::styled("it ", Style::default().fg(MUTED_GRAY)),
            Span::styled("p", Style::default().fg(CORNFLOWER_BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(" commit", Style::default().fg(MUTED_GRAY)),
            Span::styled(" │ ", Style::default().fg(SUBDUED_BORDER)),
            // Tools group
            Span::styled("c", Style::default().fg(WARNING_ORANGE).add_modifier(Modifier::BOLD)),
            Span::styled("laude ", Style::default().fg(MUTED_GRAY)),
            Span::styled("f", Style::default().fg(WARNING_ORANGE).add_modifier(Modifier::BOLD)),
            Span::styled(" refresh ", Style::default().fg(MUTED_GRAY)),
            Span::styled("x", Style::default().fg(WARNING_ORANGE).add_modifier(Modifier::BOLD)),
            Span::styled(" cleanup", Style::default().fg(MUTED_GRAY)),
            Span::styled(" │ ", Style::default().fg(SUBDUED_BORDER)),
            // System group
            Span::styled("r", Style::default().fg(MUTED_GRAY).add_modifier(Modifier::BOLD)),
            Span::styled(" re-auth ", Style::default().fg(MUTED_GRAY)),
            Span::styled("?", Style::default().fg(MUTED_GRAY).add_modifier(Modifier::BOLD)),
            Span::styled(" help ", Style::default().fg(MUTED_GRAY)),
            Span::styled("q", Style::default().fg(CORNFLOWER_BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(" home", Style::default().fg(MUTED_GRAY)),
        ];

        let menu_line = Line::from(menu_spans);

        let menu = Paragraph::new(menu_line)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(SUBDUED_BORDER))
                    .style(Style::default().bg(PANEL_BG)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(menu, area);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        let mut status_spans: Vec<Span> = vec![];

        // Current workspace/repo info
        if let Some(workspace_idx) = state.selected_workspace_index {
            if let Some(workspace) = state.workspaces.get(workspace_idx) {
                if let Some(repo_name) = workspace.path.file_name().and_then(|n| n.to_str()) {
                    status_spans.push(Span::styled("📁 ", Style::default().fg(GOLD)));
                    status_spans.push(Span::styled(repo_name.to_string(), Style::default().fg(SOFT_WHITE)));
                }
            }
        }

        // Active session info
        if let Some(_session_id) = state.get_selected_session_id() {
            if let Some(workspace_idx) = state.selected_workspace_index {
                if let Some(session_idx) = state.selected_session_index {
                    if let Some(workspace) = state.workspaces.get(workspace_idx) {
                        if let Some(session) = workspace.sessions.get(session_idx) {
                            // Separator
                            if !status_spans.is_empty() {
                                status_spans.push(Span::styled("  │  ", Style::default().fg(SUBDUED_BORDER)));
                            }

                            // Branch info
                            status_spans.push(Span::styled("🌿 ", Style::default().fg(SELECTION_GREEN)));
                            status_spans.push(Span::styled(session.branch_name.clone(), Style::default().fg(SOFT_WHITE)));

                            // Container info
                            if let Some(container_id) = &session.container_id {
                                let short_id = &container_id[..8.min(container_id.len())];
                                let (status_icon, status_color) = match session.status {
                                    crate::models::SessionStatus::Running => ("🟢", SELECTION_GREEN),
                                    crate::models::SessionStatus::Stopped => ("🔴", Color::Rgb(230, 100, 100)),
                                    crate::models::SessionStatus::Idle => ("🟡", WARNING_ORANGE),
                                    crate::models::SessionStatus::Error(_) => ("❌", Color::Rgb(230, 100, 100)),
                                };
                                status_spans.push(Span::styled("  │  ", Style::default().fg(SUBDUED_BORDER)));
                                status_spans.push(Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)));
                                status_spans.push(Span::styled(format!("{} ", session.name), Style::default().fg(SOFT_WHITE)));
                                status_spans.push(Span::styled(format!("({})", short_id), Style::default().fg(MUTED_GRAY)));
                            }
                        }
                    }
                }
            }
        }

        // Claude chat status
        if !status_spans.is_empty() {
            status_spans.push(Span::styled("  │  ", Style::default().fg(SUBDUED_BORDER)));
        }
        if state.claude_chat_visible {
            status_spans.push(Span::styled("🗨️ ", Style::default().fg(SELECTION_GREEN)));
            status_spans.push(Span::styled("ON", Style::default().fg(SELECTION_GREEN)));
        } else {
            status_spans.push(Span::styled("🗨️ ", Style::default().fg(MUTED_GRAY)));
            status_spans.push(Span::styled("OFF", Style::default().fg(MUTED_GRAY)));
        }

        let status_line = if status_spans.is_empty() {
            Line::from(Span::styled("Agents-in-a-Box - No active session", Style::default().fg(MUTED_GRAY)))
        } else {
            Line::from(status_spans)
        };

        let status = Paragraph::new(status_line)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(CORNFLOWER_BLUE))
                    .style(Style::default().bg(DARK_BG))
                    .title(Line::from(vec![
                        Span::styled(" 📊 ", Style::default().fg(GOLD)),
                        Span::styled("Status", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                    ])),
            )
            .alignment(Alignment::Left);

        frame.render_widget(status, area);
    }

    fn render_notifications(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        let notifications = state.get_current_notifications();
        if notifications.is_empty() {
            return;
        }

        // Position notifications in the top-right corner
        let notification_width = 50;
        let notification_height = notifications.len() as u16 * 3; // 3 lines per notification

        let notification_area = Rect {
            x: area.width.saturating_sub(notification_width + 2),
            y: 1,
            width: notification_width,
            height: notification_height.min(area.height.saturating_sub(2)),
        };

        // Render each notification
        for (i, notification) in notifications.iter().enumerate() {
            let y_offset = i as u16 * 3;
            if y_offset >= notification_area.height {
                break; // Don't render notifications that won't fit
            }

            let single_notification_area = Rect {
                x: notification_area.x,
                y: notification_area.y + y_offset,
                width: notification_area.width,
                height: 3.min(notification_area.height - y_offset),
            };

            let (icon, text_color, border_color) = match notification.notification_type {
                crate::app::state::NotificationType::Success => {
                    ("✓ ", SELECTION_GREEN, SELECTION_GREEN)
                }
                crate::app::state::NotificationType::Error => {
                    ("✗ ", Color::Rgb(230, 100, 100), Color::Rgb(230, 100, 100))
                }
                crate::app::state::NotificationType::Warning => {
                    ("⚠ ", WARNING_ORANGE, WARNING_ORANGE)
                }
                crate::app::state::NotificationType::Info => {
                    ("ℹ ", CORNFLOWER_BLUE, CORNFLOWER_BLUE)
                }
            };

            let notification_line = Line::from(vec![
                Span::styled(icon, Style::default().fg(text_color).add_modifier(Modifier::BOLD)),
                Span::styled(notification.message.as_str(), Style::default().fg(text_color)),
            ]);

            let notification_widget = Paragraph::new(notification_line)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(PANEL_BG)),
                )
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(notification_widget, single_notification_area);
        }
    }

    fn render_quick_commit_dialog(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        // Create a centered dialog area
        let dialog_area = centered_rect(60, 20, area);

        // Clear the background with premium dark bg
        let clear = Block::default().style(Style::default().bg(DARK_BG));
        frame.render_widget(clear, dialog_area);

        // Create the dialog layout
        let dialog_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Input field
                Constraint::Length(2), // Instructions
            ])
            .split(dialog_area);

        // Render title
        let title = Paragraph::new(Line::from(vec![
            Span::styled("🚀 ", Style::default().fg(GOLD)),
            Span::styled("Quick Commit", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(CORNFLOWER_BLUE))
                .style(Style::default().bg(DARK_BG))
                .title(Line::from(vec![
                    Span::styled(" 📋 ", Style::default().fg(GOLD)),
                    Span::styled("Git Commit", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                ])),
        )
        .alignment(Alignment::Center);
        frame.render_widget(title, dialog_layout[0]);

        // Render input field with block cursor
        let empty_string = String::new();
        let commit_message = state.quick_commit_message.as_ref().unwrap_or(&empty_string);

        // Create spans with cursor visualization
        let (before_cursor, after_cursor) = commit_message.split_at(
            state.quick_commit_cursor.min(commit_message.len())
        );

        let input_line = Line::from(vec![
            Span::styled(before_cursor, Style::default().fg(SOFT_WHITE)),
            Span::styled("█", Style::default().fg(SELECTION_GREEN)),
            Span::styled(after_cursor, Style::default().fg(SOFT_WHITE)),
        ]);

        let input_paragraph = Paragraph::new(input_line)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(SELECTION_GREEN))
                    .style(Style::default().bg(Color::Rgb(35, 35, 45)))
                    .title(Line::from(vec![
                        Span::styled(" ✏️ ", Style::default().fg(GOLD)),
                        Span::styled("Commit Message", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                    ])),
            );
        frame.render_widget(input_paragraph, dialog_layout[1]);

        // Render instructions
        let instructions = Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(SELECTION_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" Commit & Push ", Style::default().fg(MUTED_GRAY)),
            Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
            Span::styled(" Esc", Style::default().fg(WARNING_ORANGE).add_modifier(Modifier::BOLD)),
            Span::styled(" Cancel", Style::default().fg(MUTED_GRAY)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(instructions, dialog_layout[2]);
    }
}

impl Default for LayoutComponent {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
