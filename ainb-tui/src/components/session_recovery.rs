// ABOUTME: Session recovery component for recovering orphaned agent sessions after crash/shutdown
// Displays orphaned sessions (tmux dead, worktree exists) and allows resume/cleanup actions

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;


// Color palette (matching TUI style guide)
const CORNFLOWER_BLUE: Color = Color::Rgb(100, 149, 237);
const GOLD: Color = Color::Rgb(255, 215, 0);
const SELECTION_GREEN: Color = Color::Rgb(100, 200, 100);
const WARNING_ORANGE: Color = Color::Rgb(255, 165, 0);
const DARK_BG: Color = Color::Rgb(25, 25, 35);
const PANEL_BG: Color = Color::Rgb(30, 30, 40);
const LIST_HIGHLIGHT_BG: Color = Color::Rgb(40, 40, 60);
const SOFT_WHITE: Color = Color::Rgb(220, 220, 230);
const MUTED_GRAY: Color = Color::Rgb(120, 120, 140);
const SUBDUED_BORDER: Color = Color::Rgb(60, 60, 80);

/// Represents an orphaned agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanedSession {
    pub session: String,
    pub task: String,
    pub directory: String,
    pub created: String,
    pub status: String,
    pub transcript_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub can_resume: bool,
    pub time_ago: String,
}

/// State for the session recovery component
#[derive(Debug, Clone)]
pub struct SessionRecoveryState {
    pub orphaned_sessions: Vec<OrphanedSession>,
    pub selected_index: usize,
    pub list_state: ListState,
    pub loading: bool,
    pub last_error: Option<String>,
    pub action_result: Option<String>,
}

impl Default for SessionRecoveryState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRecoveryState {
    pub fn new() -> Self {
        let mut state = Self {
            orphaned_sessions: Vec::new(),
            selected_index: 0,
            list_state: ListState::default(),
            loading: true,
            last_error: None,
            action_result: None,
        };
        state.refresh();
        state
    }

    /// Refresh the list of orphaned sessions
    pub fn refresh(&mut self) {
        self.loading = true;
        self.last_error = None;
        self.action_result = None;

        match Self::load_orphaned_sessions() {
            Ok(sessions) => {
                self.orphaned_sessions = sessions;
                self.loading = false;
                if !self.orphaned_sessions.is_empty() && self.selected_index >= self.orphaned_sessions.len() {
                    self.selected_index = self.orphaned_sessions.len() - 1;
                }
                self.list_state.select(if self.orphaned_sessions.is_empty() {
                    None
                } else {
                    Some(self.selected_index)
                });
            }
            Err(e) => {
                self.last_error = Some(e);
                self.loading = false;
            }
        }
    }

    /// Load orphaned sessions from ~/.claude/agents/
    fn load_orphaned_sessions() -> Result<Vec<OrphanedSession>, String> {
        let agents_dir = dirs::home_dir()
            .ok_or("Could not find home directory")?
            .join(".claude")
            .join("agents");

        if !agents_dir.exists() {
            return Ok(Vec::new());
        }

        let mut orphaned = Vec::new();

        for entry in std::fs::read_dir(&agents_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if path.file_name().map(|n| n == "registry.jsonl").unwrap_or(false) {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                        let session = meta["session"].as_str().unwrap_or("").to_string();
                        let status = meta["status"].as_str().unwrap_or("unknown").to_string();

                        // Skip completed/archived sessions
                        if status == "completed" || status == "archived" {
                            continue;
                        }

                        // Check if tmux session exists
                        let tmux_alive = Command::new("tmux")
                            .args(["has-session", "-t", &session])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);

                        if tmux_alive {
                            continue; // Not orphaned
                        }

                        // Check if worktree exists
                        let directory = meta["directory"].as_str().unwrap_or("").to_string();
                        if directory.is_empty() || !PathBuf::from(&directory).exists() {
                            continue;
                        }

                        // Check for transcript
                        let transcript_path = meta["transcript_path"].as_str().map(|s| s.to_string());
                        let can_resume = transcript_path
                            .as_ref()
                            .map(|p| PathBuf::from(p).exists())
                            .unwrap_or(false);

                        // Calculate time ago
                        let created = meta["created"].as_str().unwrap_or("").to_string();
                        let time_ago = Self::calculate_time_ago(&created);

                        orphaned.push(OrphanedSession {
                            session,
                            task: meta["task"].as_str().unwrap_or("Unknown task").to_string(),
                            directory,
                            created,
                            status,
                            transcript_path,
                            worktree_branch: meta["worktree_branch"].as_str().map(|s| s.to_string()),
                            can_resume,
                            time_ago,
                        });
                    }
                }
            }
        }

        // Sort by created date (newest first)
        orphaned.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(orphaned)
    }

    fn calculate_time_ago(created: &str) -> String {
        use chrono::{DateTime, Utc};

        if let Ok(dt) = DateTime::parse_from_rfc3339(created) {
            let now = Utc::now();
            let duration = now.signed_duration_since(dt.with_timezone(&Utc));

            let hours = duration.num_hours();
            if hours < 1 {
                let minutes = duration.num_minutes();
                return format!("{}m ago", minutes);
            } else if hours < 24 {
                return format!("{}h ago", hours);
            } else {
                let days = hours / 24;
                return format!("{}d ago", days);
            }
        }

        String::new()
    }

    pub fn next(&mut self) {
        if self.orphaned_sessions.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.orphaned_sessions.len();
        self.list_state.select(Some(self.selected_index));
    }

    pub fn previous(&mut self) {
        if self.orphaned_sessions.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.orphaned_sessions.len() - 1;
        } else {
            self.selected_index -= 1;
        }
        self.list_state.select(Some(self.selected_index));
    }

    pub fn selected(&self) -> Option<&OrphanedSession> {
        self.orphaned_sessions.get(self.selected_index)
    }

    /// Resume the selected session
    pub fn resume_selected(&mut self) -> Result<String, String> {
        let session = self.selected().ok_or("No session selected")?;

        if !session.can_resume {
            return Err("Cannot resume: no transcript found".to_string());
        }

        let transcript = session
            .transcript_path
            .as_ref()
            .ok_or("No transcript path")?;

        let new_session = format!("{}-resumed-{}", session.session, chrono::Utc::now().timestamp());
        let directory = &session.directory;

        // Create new tmux session
        let create_result = Command::new("tmux")
            .args(["new-session", "-d", "-s", &new_session, "-c", directory])
            .output()
            .map_err(|e| e.to_string())?;

        if !create_result.status.success() {
            return Err(format!(
                "Failed to create tmux session: {}",
                String::from_utf8_lossy(&create_result.stderr)
            ));
        }

        // Start Claude with --resume
        let claude_cmd = format!(
            "claude --dangerously-skip-permissions --resume \"{}\"",
            transcript
        );
        let send_result = Command::new("tmux")
            .args(["send-keys", "-t", &new_session, &claude_cmd, "C-m"])
            .output()
            .map_err(|e| e.to_string())?;

        if !send_result.status.success() {
            return Err(format!(
                "Failed to start Claude: {}",
                String::from_utf8_lossy(&send_result.stderr)
            ));
        }

        self.action_result = Some(format!("Resumed as: {}", new_session));
        self.refresh();

        Ok(new_session)
    }

    /// Archive the selected session
    pub fn archive_selected(&mut self) -> Result<(), String> {
        let session = self.selected().ok_or("No session selected")?.clone();

        let agents_dir = dirs::home_dir()
            .ok_or("Could not find home directory")?
            .join(".claude")
            .join("agents");

        let archived_dir = agents_dir.join("archived");
        std::fs::create_dir_all(&archived_dir).map_err(|e| e.to_string())?;

        let meta_file = agents_dir.join(format!("{}.json", session.session));
        let archived_file = archived_dir.join(format!("{}.json", session.session));

        if meta_file.exists() {
            // Update status to archived
            if let Ok(content) = std::fs::read_to_string(&meta_file) {
                if let Ok(mut meta) = serde_json::from_str::<serde_json::Value>(&content) {
                    meta["status"] = serde_json::Value::String("archived".to_string());
                    meta["archived_at"] =
                        serde_json::Value::String(chrono::Utc::now().to_rfc3339());

                    std::fs::write(&archived_file, serde_json::to_string_pretty(&meta).unwrap())
                        .map_err(|e| e.to_string())?;
                    std::fs::remove_file(&meta_file).map_err(|e| e.to_string())?;
                }
            }
        }

        self.action_result = Some(format!("Archived: {}", session.session));
        self.refresh();

        Ok(())
    }
}

/// Session recovery component renderer
pub struct SessionRecovery;

impl SessionRecovery {
    pub fn render(frame: &mut Frame, area: Rect, state: &mut SessionRecoveryState) {
        // Main layout: list on left, details on right
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);

        Self::render_session_list(frame, chunks[0], state);
        Self::render_session_details(frame, chunks[1], state);
    }

    fn render_session_list(frame: &mut Frame, area: Rect, state: &mut SessionRecoveryState) {
        let orphan_count = state.orphaned_sessions.len();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(CORNFLOWER_BLUE))
            .style(Style::default().bg(DARK_BG))
            .title(Line::from(vec![
                Span::styled(" ", Style::default().fg(GOLD)),
                Span::styled(
                    "Orphaned Sessions ",
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", orphan_count),
                    Style::default()
                        .fg(if orphan_count > 0 {
                            WARNING_ORANGE
                        } else {
                            SELECTION_GREEN
                        })
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
            .title_bottom(Line::from(vec![
                Span::styled(" ", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled("/", Style::default().fg(MUTED_GRAY)),
                Span::styled("", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" navigate ", Style::default().fg(MUTED_GRAY)),
                Span::styled("|", Style::default().fg(SUBDUED_BORDER)),
                Span::styled(" r", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" resume ", Style::default().fg(MUTED_GRAY)),
                Span::styled("|", Style::default().fg(SUBDUED_BORDER)),
                Span::styled(" d", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" archive ", Style::default().fg(MUTED_GRAY)),
                Span::styled("|", Style::default().fg(SUBDUED_BORDER)),
                Span::styled(" R", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" refresh ", Style::default().fg(MUTED_GRAY)),
            ]));

        if state.loading {
            let loading = Paragraph::new("Loading sessions...")
                .style(Style::default().fg(MUTED_GRAY))
                .block(block);
            frame.render_widget(loading, area);
            return;
        }

        if state.orphaned_sessions.is_empty() {
            let empty_state = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    " No orphaned sessions found",
                    Style::default().fg(SELECTION_GREEN),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "All agent sessions are either active or cleaned up.",
                    Style::default().fg(MUTED_GRAY).add_modifier(Modifier::ITALIC),
                )),
            ])
            .block(block);
            frame.render_widget(empty_state, area);
            return;
        }

        let items: Vec<ListItem> = state
            .orphaned_sessions
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let is_selected = i == state.selected_index;

                let resume_indicator = if session.can_resume { "" } else { "" };
                let time_indicator = if session.time_ago.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", session.time_ago)
                };

                let task_preview: String = session
                    .task
                    .chars()
                    .take(30)
                    .collect::<String>()
                    .replace('\n', " ");

                let mut spans = vec![];
                if is_selected {
                    spans.push(Span::styled(" ", Style::default().fg(SELECTION_GREEN)));
                } else {
                    spans.push(Span::raw("  "));
                }

                spans.push(Span::styled(
                    resume_indicator,
                    if session.can_resume {
                        Style::default().fg(SELECTION_GREEN)
                    } else {
                        Style::default().fg(MUTED_GRAY)
                    },
                ));
                spans.push(Span::raw(" "));

                spans.push(Span::styled(
                    &session.session,
                    if is_selected {
                        Style::default()
                            .fg(SELECTION_GREEN)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(SOFT_WHITE)
                    },
                ));

                spans.push(Span::styled(
                    time_indicator,
                    Style::default().fg(MUTED_GRAY),
                ));

                let base_style = if is_selected {
                    Style::default().bg(LIST_HIGHLIGHT_BG)
                } else {
                    Style::default()
                };

                ListItem::new(vec![
                    Line::from(spans),
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(format!("{}...", task_preview), Style::default().fg(MUTED_GRAY)),
                    ]),
                ])
                .style(base_style)
            })
            .collect();

        let list = List::new(items).block(block);

        frame.render_stateful_widget(list, area, &mut state.list_state);
    }

    fn render_session_details(frame: &mut Frame, area: Rect, state: &SessionRecoveryState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(CORNFLOWER_BLUE))
            .style(Style::default().bg(DARK_BG))
            .title(Line::from(vec![
                Span::styled(" ", Style::default().fg(GOLD)),
                Span::styled(
                    "Session Details",
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                ),
            ]));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Show action result if any
        if let Some(ref result) = state.action_result {
            let result_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(inner);

            let result_widget = Paragraph::new(Line::from(vec![
                Span::styled(" ", Style::default().fg(SELECTION_GREEN)),
                Span::styled(result, Style::default().fg(SELECTION_GREEN)),
            ]))
            .wrap(Wrap { trim: true });

            frame.render_widget(result_widget, result_area[0]);

            if let Some(session) = state.selected() {
                Self::render_session_info(frame, result_area[1], session);
            }
            return;
        }

        // Show error if any
        if let Some(ref error) = state.last_error {
            let error_widget = Paragraph::new(Line::from(vec![
                Span::styled(" Error: ", Style::default().fg(WARNING_ORANGE)),
                Span::styled(error, Style::default().fg(SOFT_WHITE)),
            ]))
            .wrap(Wrap { trim: true });

            frame.render_widget(error_widget, inner);
            return;
        }

        // Show selected session details
        if let Some(session) = state.selected() {
            Self::render_session_info(frame, inner, session);
        } else {
            let empty = Paragraph::new(Span::styled(
                "Select a session to view details",
                Style::default().fg(MUTED_GRAY),
            ));
            frame.render_widget(empty, inner);
        }
    }

    fn render_session_info(frame: &mut Frame, area: Rect, session: &OrphanedSession) {
        let lines = vec![
            Line::from(vec![
                Span::styled("Session:  ", Style::default().fg(MUTED_GRAY)),
                Span::styled(&session.session, Style::default().fg(SOFT_WHITE)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Status:   ", Style::default().fg(MUTED_GRAY)),
                Span::styled(
                    if session.can_resume {
                        " Resumable"
                    } else {
                        " No transcript"
                    },
                    if session.can_resume {
                        Style::default().fg(SELECTION_GREEN)
                    } else {
                        Style::default().fg(WARNING_ORANGE)
                    },
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Created:  ", Style::default().fg(MUTED_GRAY)),
                Span::styled(&session.created, Style::default().fg(SOFT_WHITE)),
                if !session.time_ago.is_empty() {
                    Span::styled(
                        format!(" ({})", session.time_ago),
                        Style::default().fg(MUTED_GRAY),
                    )
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Directory:", Style::default().fg(MUTED_GRAY)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(&session.directory, Style::default().fg(CORNFLOWER_BLUE)),
            ]),
            Line::from(""),
            if let Some(ref branch) = session.worktree_branch {
                Line::from(vec![
                    Span::styled("Branch:   ", Style::default().fg(MUTED_GRAY)),
                    Span::styled(branch, Style::default().fg(SOFT_WHITE)),
                ])
            } else {
                Line::from("")
            },
            Line::from(""),
            Line::from(vec![
                Span::styled("Task:", Style::default().fg(MUTED_GRAY)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    session.task.chars().take(200).collect::<String>(),
                    Style::default().fg(SOFT_WHITE),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(PANEL_BG));

        frame.render_widget(paragraph, area);
    }
}
