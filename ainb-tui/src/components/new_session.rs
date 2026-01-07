// ABOUTME: New session creation UI component with repository selection and branch input

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::app::{
    AppState,
    state::{NewSessionState, NewSessionStep},
};

pub struct NewSessionComponent {
    search_list_state: ListState,
}

impl NewSessionComponent {
    pub fn new() -> Self {
        Self {
            search_list_state: ListState::default(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        if let Some(ref session_state) = state.new_session_state {
            // Create a centered popup
            let popup_area = self.centered_rect(80, 70, area);

            // Clear the background
            frame.render_widget(Clear, popup_area);

            match session_state.step {
                NewSessionStep::SelectRepo => {
                    if state.current_view == crate::app::state::View::SearchWorkspace {
                        self.render_search_workspace(frame, popup_area, session_state)
                    } else {
                        self.render_repo_selection(frame, popup_area, session_state)
                    }
                }
                NewSessionStep::SelectAgent => {
                    self.render_agent_selection(frame, popup_area, session_state)
                }
                NewSessionStep::InputBranch => {
                    self.render_branch_input(frame, popup_area, session_state)
                }
                NewSessionStep::SelectMode => {
                    self.render_mode_selection(frame, popup_area, session_state)
                }
                NewSessionStep::InputPrompt => {
                    self.render_prompt_input(frame, popup_area, session_state)
                }
                NewSessionStep::ConfigurePermissions => {
                    self.render_permissions_config(frame, popup_area, session_state)
                }
                NewSessionStep::Creating => self.render_creating(frame, popup_area),
            }
        }
    }

    fn render_repo_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Modern color palette
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(120, 120, 140);
        let selection_green = Color::Rgb(100, 200, 100);
        let subdued_border = Color::Rgb(60, 60, 80);
        let list_highlight_bg = Color::Rgb(40, 40, 60);

        // Clear background
        let background = Block::default().style(Style::default().bg(dark_bg));
        frame.render_widget(background, area);

        // Main dialog with rounded border
        let title_line = Line::from(vec![
            Span::styled(" 📁 ", Style::default().fg(gold)),
            Span::styled("Select Repository", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(title_line)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(2), // Subtitle
                Constraint::Min(0),    // Repository list
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Footer
            ])
            .split(inner);

        // Subtitle
        let subtitle = Paragraph::new(Line::from(vec![
            Span::styled("Choose a repository to create a new session", Style::default().fg(muted_gray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(subtitle, chunks[0]);

        // Repository list with modern styling
        let repos: Vec<ListItem> = if session_state.filtered_repos.is_empty() {
            vec![
                ListItem::new(Line::from(vec![
                    Span::styled("  ⚠️  ", Style::default().fg(gold)),
                    Span::styled("No repositories found in default paths", Style::default().fg(muted_gray)),
                ])),
                ListItem::new(Line::from(vec![
                    Span::styled("     Try searching in common directories like:", Style::default().fg(muted_gray)),
                ])),
                ListItem::new(Line::from(vec![
                    Span::styled("     ~/projects, ~/code, ~/dev, ~/src", Style::default().fg(cornflower_blue)),
                ])),
                ListItem::new(Line::from(vec![
                    Span::styled("  💡 ", Style::default().fg(gold)),
                    Span::styled("Type to filter or add custom paths", Style::default().fg(gold)),
                ])),
            ]
        } else {
            session_state
                .filtered_repos
                .iter()
                .enumerate()
                .map(|(display_idx, (_, repo))| {
                    let repo_name = repo.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

                    if Some(display_idx) == session_state.selected_repo_index {
                        ListItem::new(Line::from(vec![
                            Span::styled("  ▶ ", Style::default().fg(selection_green)),
                            Span::styled(repo_name, Style::default().fg(selection_green).add_modifier(Modifier::BOLD)),
                        ]))
                    } else {
                        ListItem::new(Line::from(vec![
                            Span::styled("    ", Style::default()),
                            Span::styled(repo_name, Style::default().fg(soft_white)),
                        ]))
                    }
                })
                .collect()
        };

        let repo_count = session_state.filtered_repos.len();
        let list_title = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(format!("Repositories ({})", repo_count), Style::default().fg(cornflower_blue)),
            Span::styled(" ", Style::default()),
        ]);

        let repo_list = List::new(repos)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(subdued_border))
                    .title(list_title)
                    .style(Style::default().bg(dark_bg)),
            )
            .highlight_style(Style::default().bg(list_highlight_bg));

        frame.render_widget(repo_list, chunks[1]);

        // Modern footer with keyboard hints (simplified - no agent/model here)
        let footer_spans = vec![
            Span::styled("↑↓", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" Repos", Style::default().fg(muted_gray)),
            Span::styled("  │  ", Style::default().fg(subdued_border)),
            Span::styled("Enter", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" Select", Style::default().fg(muted_gray)),
            Span::styled("  │  ", Style::default().fg(subdued_border)),
            Span::styled("Esc", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled(" Cancel", Style::default().fg(muted_gray)),
        ];

        let footer = Paragraph::new(Line::from(footer_spans))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[3]);
    }

    fn render_search_workspace(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Color palette
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let muted_gray = Color::Rgb(128, 128, 128);
        let selection_green = Color::Rgb(100, 200, 100);
        let subdued_border = Color::Rgb(70, 70, 90);
        let list_highlight_bg = Color::Rgb(45, 45, 60);

        // Draw outer border with gradient-like effect using rounded corners
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(Span::styled(
                " 🔍 Search Repositories ",
                Style::default()
                    .fg(gold)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Length(3), // Search input
                Constraint::Length(1), // Spacer
                Constraint::Min(0),    // Repository list
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Instructions
            ])
            .split(inner);

        // Search input with icon and styled placeholder
        let search_text = if session_state.filter_text.is_empty() {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "Type to search repositories...",
                    Style::default().fg(muted_gray).add_modifier(Modifier::ITALIC),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("  ", Style::default().fg(selection_green)),
                Span::styled(
                    &session_state.filter_text,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█", Style::default().fg(selection_green)), // Cursor
            ])
        };

        let search_input = Paragraph::new(search_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(selection_green))
                    .style(Style::default().bg(Color::Rgb(35, 35, 45))),
            );
        frame.render_widget(search_input, chunks[0]);

        // Repository list with enhanced styling
        let total_repos = session_state.available_repos.len();
        let filtered_count = session_state.filtered_repos.len();

        let repos: Vec<ListItem> = session_state
            .filtered_repos
            .iter()
            .enumerate()
            .map(|(display_idx, (_, repo))| {
                let repo_name = repo.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let parent_path = repo.parent()
                    .and_then(|p| p.to_str())
                    .map(|s| {
                        // Truncate long paths
                        if s.len() > 50 {
                            format!("...{}", &s[s.len()-47..])
                        } else {
                            s.to_string()
                        }
                    })
                    .unwrap_or_default();

                let is_selected = Some(display_idx) == session_state.selected_repo_index;

                if is_selected {
                    // Selected item - highlighted with arrow and full styling
                    let lines = vec![
                        Line::from(vec![
                            Span::styled("  ▶ ", Style::default().fg(gold)),
                            Span::styled("📁 ", Style::default()),
                            Span::styled(
                                repo_name,
                                Style::default()
                                    .fg(gold)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("      ", Style::default()),
                            Span::styled(
                                parent_path,
                                Style::default().fg(Color::Rgb(150, 150, 150)).add_modifier(Modifier::ITALIC),
                            ),
                        ]),
                    ];
                    ListItem::new(lines).style(Style::default().bg(list_highlight_bg))
                } else {
                    // Non-selected item
                    let lines = vec![
                        Line::from(vec![
                            Span::styled("    ", Style::default()),
                            Span::styled("📂 ", Style::default()),
                            Span::styled(
                                repo_name,
                                Style::default().fg(Color::Rgb(200, 200, 200)),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("      ", Style::default()),
                            Span::styled(
                                parent_path,
                                Style::default().fg(Color::Rgb(100, 100, 100)),
                            ),
                        ]),
                    ];
                    ListItem::new(lines)
                }
            })
            .collect();

        // Title with count badge
        let count_style = if filtered_count < total_repos {
            Style::default().fg(Color::Rgb(255, 165, 0)) // Orange when filtered
        } else {
            Style::default().fg(selection_green) // Green when showing all
        };

        let title_spans = vec![
            Span::styled(" Repositories ", Style::default().fg(Color::Rgb(200, 200, 200))),
            Span::styled(
                format!("({}/{})", filtered_count, total_repos),
                count_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ];

        let repo_list = List::new(repos)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(subdued_border))
                    .title(Line::from(title_spans))
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
            );

        // Update the list state to match the current selection
        self.search_list_state.select(session_state.selected_repo_index);

        frame.render_stateful_widget(repo_list, chunks[2], &mut self.search_list_state);

        // Styled instructions footer (simplified - no agent/model here)
        let instruction_spans = vec![
            Span::styled("  ⌨️  ", Style::default()),
            Span::styled("Type", Style::default().fg(selection_green)),
            Span::styled(" filter  ", Style::default().fg(muted_gray)),
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled("  ↑↓ ", Style::default().fg(selection_green)),
            Span::styled("Repos  ", Style::default().fg(muted_gray)),
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled("  ⏎ ", Style::default().fg(selection_green)),
            Span::styled("Select  ", Style::default().fg(muted_gray)),
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel", Style::default().fg(muted_gray)),
        ];

        let instructions = Line::from(instruction_spans);
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(instructions_widget, chunks[4]);
    }

    /// Render a compact inline agent selector (single line)
    fn render_inline_agent_selector(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
        colors: &[Color],
    ) {
        let [gold, soft_white, muted_gray, selection_green, _list_highlight_bg] = colors[..] else {
            return;
        };

        let mut spans = vec![
            Span::styled("Agent: ", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
        ];

        for (idx, option) in session_state.agent_options.iter().enumerate() {
            let is_selected = idx == session_state.selected_agent_index;
            let agent = option.agent_type;
            let is_available = agent.is_available();

            if idx > 0 {
                spans.push(Span::styled("  │  ", Style::default().fg(muted_gray)));
            }

            if is_selected {
                spans.push(Span::styled("▶ ", Style::default().fg(selection_green)));
                spans.push(Span::styled(
                    format!("{} ", agent.icon()),
                    Style::default().fg(soft_white),
                ));
                spans.push(Span::styled(
                    agent.name(),
                    Style::default().fg(selection_green).add_modifier(Modifier::BOLD),
                ));
            } else if is_available {
                spans.push(Span::styled(
                    format!("{} {}", agent.icon(), agent.name()),
                    Style::default().fg(soft_white),
                ));
            } else {
                spans.push(Span::styled(
                    format!("{} {} [Soon]", agent.icon(), agent.name()),
                    Style::default().fg(Color::Rgb(80, 80, 100)),
                ));
            }
        }

        let agent_line = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center);
        frame.render_widget(agent_line, area);
    }

    /// Render a compact inline model selector (single line)
    fn render_inline_model_selector(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
        colors: &[Color],
    ) {
        let [gold, soft_white, muted_gray, selection_green, _list_highlight_bg] = colors[..] else {
            return;
        };

        let mut spans = vec![
            Span::styled("Model: ", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
        ];

        for (idx, model) in session_state.model_options.iter().enumerate() {
            let is_selected = idx == session_state.selected_model_index;

            if idx > 0 {
                spans.push(Span::styled("  │  ", Style::default().fg(muted_gray)));
            }

            if is_selected {
                spans.push(Span::styled("◀ ", Style::default().fg(selection_green)));
                spans.push(Span::styled(
                    format!("{} ", model.icon()),
                    Style::default().fg(soft_white),
                ));
                spans.push(Span::styled(
                    model.display_name(),
                    Style::default().fg(selection_green).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(" ▶", Style::default().fg(selection_green)));
            } else {
                spans.push(Span::styled(
                    format!("{} {}", model.icon(), model.display_name()),
                    Style::default().fg(soft_white),
                ));
            }
        }

        let model_line = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center);
        frame.render_widget(model_line, area);
    }

    fn render_agent_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Modern color palette (from TUI style guide)
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(120, 120, 140);
        let selection_green = Color::Rgb(100, 200, 100);
        let subdued_border = Color::Rgb(60, 60, 80);
        let coming_soon_gray = Color::Rgb(80, 80, 100);
        let list_highlight_bg = Color::Rgb(40, 40, 60);

        // Draw outer border with modern styling
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(Span::styled(
                " 🤖 Select Agent ",
                Style::default()
                    .fg(gold)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        // Determine if model selection should be shown
        let show_model = session_state.should_show_model_selection();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(if show_model {
                vec![
                    Constraint::Length(2), // Header text
                    Constraint::Length(1), // Spacer
                    Constraint::Min(0),    // Agent list
                    Constraint::Length(1), // Spacer
                    Constraint::Length(3), // Model selection bar
                    Constraint::Length(1), // Spacer
                    Constraint::Length(2), // Instructions
                ]
            } else {
                vec![
                    Constraint::Length(2), // Header text
                    Constraint::Length(1), // Spacer
                    Constraint::Min(0),    // Agent list
                    Constraint::Length(1), // Spacer
                    Constraint::Length(2), // Instructions
                ]
            })
            .split(inner);

        // Header text
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Choose an AI agent for this session", Style::default().fg(soft_white)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(header, chunks[0]);

        // Agent list
        let agent_area = chunks[2];
        self.render_agent_list(frame, agent_area, session_state, &[
            cornflower_blue, dark_bg, gold, soft_white, muted_gray, selection_green,
            subdued_border, coming_soon_gray, list_highlight_bg,
        ]);

        // Model selection bar (if Claude is selected)
        if show_model {
            self.render_model_selector(frame, chunks[4], session_state, &[
                cornflower_blue, dark_bg, gold, soft_white, muted_gray, selection_green,
                subdued_border, list_highlight_bg,
            ]);
        }

        // Instructions
        let instruction_area = if show_model { chunks[6] } else { chunks[4] };
        let mut instruction_spans = vec![
            Span::styled(" ↑↓ ", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled("Agent  ", Style::default().fg(muted_gray)),
        ];

        if show_model {
            instruction_spans.extend(vec![
                Span::styled("│", Style::default().fg(subdued_border)),
                Span::styled(" ←→ ", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled("Model  ", Style::default().fg(muted_gray)),
            ]);
        }

        instruction_spans.extend(vec![
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled(" Enter ", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled("Confirm  ", Style::default().fg(muted_gray)),
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled(" Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel", Style::default().fg(muted_gray)),
        ]);

        let instructions = Line::from(instruction_spans);
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(instructions_widget, instruction_area);
    }

    fn render_agent_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
        colors: &[Color],
    ) {
        let [_cornflower_blue, dark_bg, _gold, soft_white, muted_gray, selection_green,
            subdued_border, coming_soon_gray, list_highlight_bg] = colors[..] else {
            return;
        };

        // Build agent list items
        let items: Vec<ListItem> = session_state
            .agent_options
            .iter()
            .enumerate()
            .map(|(idx, option)| {
                let is_selected = idx == session_state.selected_agent_index;
                let agent = option.agent_type;
                let is_available = agent.is_available();

                let mut spans = vec![];

                // Selection indicator
                if is_selected {
                    spans.push(Span::styled("▶ ", Style::default().fg(selection_green)));
                } else {
                    spans.push(Span::raw("  "));
                }

                // Icon
                spans.push(Span::styled(
                    format!("{} ", agent.icon()),
                    Style::default().fg(if is_available { soft_white } else { coming_soon_gray }),
                ));

                // Name
                let name_style = if is_selected && is_available {
                    Style::default().fg(selection_green).add_modifier(Modifier::BOLD)
                } else if is_available {
                    Style::default().fg(soft_white)
                } else {
                    Style::default().fg(coming_soon_gray)
                };
                spans.push(Span::styled(agent.name(), name_style));

                // Coming Soon badge
                if !is_available {
                    spans.push(Span::styled(
                        " [Soon]",
                        Style::default().fg(coming_soon_gray).add_modifier(Modifier::ITALIC),
                    ));
                }

                // Current agent indicator
                if option.is_current && is_available {
                    spans.push(Span::styled(
                        " [Current]",
                        Style::default().fg(selection_green),
                    ));
                }

                // Description on a new line for selected items
                let base_style = if is_selected {
                    Style::default().bg(list_highlight_bg)
                } else {
                    Style::default()
                };

                let lines = if is_selected {
                    vec![
                        Line::from(spans),
                        Line::from(vec![
                            Span::raw("     "),
                            Span::styled(
                                agent.description(),
                                Style::default().fg(muted_gray).add_modifier(Modifier::ITALIC),
                            ),
                        ]),
                    ]
                } else {
                    vec![Line::from(spans)]
                };

                ListItem::new(lines).style(base_style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(subdued_border))
                    .style(Style::default().bg(dark_bg)),
            )
            .highlight_style(Style::default().bg(list_highlight_bg));

        frame.render_widget(list, area);
    }

    /// Render a compact horizontal model selector bar
    fn render_model_selector(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
        colors: &[Color],
    ) {
        let [cornflower_blue, dark_bg, gold, soft_white, muted_gray, selection_green,
            _subdued_border, list_highlight_bg] = colors[..] else {
            return;
        };

        // Create the model selector block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(Span::styled(" Model ", Style::default().fg(gold)))
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        let inner = block.inner(area);

        // Build horizontal model options
        let mut spans = vec![Span::raw(" ")];

        for (idx, model) in session_state.model_options.iter().enumerate() {
            let is_selected = idx == session_state.selected_model_index;

            if idx > 0 {
                spans.push(Span::styled("  │  ", Style::default().fg(muted_gray)));
            }

            // Selection bracket and icon
            if is_selected {
                spans.push(Span::styled("◀ ", Style::default().fg(selection_green)));
                spans.push(Span::styled(
                    format!("{} ", model.icon()),
                    Style::default().fg(soft_white),
                ));
                spans.push(Span::styled(
                    model.display_name(),
                    Style::default().fg(selection_green).add_modifier(Modifier::BOLD).bg(list_highlight_bg),
                ));
                spans.push(Span::styled(" ▶", Style::default().fg(selection_green)));
            } else {
                spans.push(Span::styled(
                    format!("{} ", model.icon()),
                    Style::default().fg(muted_gray),
                ));
                spans.push(Span::styled(
                    model.display_name(),
                    Style::default().fg(soft_white),
                ));
            }
        }

        let model_line = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(model_line, inner);
    }

    fn render_branch_input(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        // Modern color palette
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(128, 128, 128);
        let selection_green = Color::Rgb(100, 200, 100);
        let subdued_border = Color::Rgb(70, 70, 90);
        let list_highlight_bg = Color::Rgb(40, 40, 60);

        // Draw outer border with modern styling
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(Span::styled(
                " 🌿 New Session ",
                Style::default()
                    .fg(gold)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        // Check if model selector should be shown
        let show_model = session_state.should_show_model_selection();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(if show_model {
                vec![
                    Constraint::Length(6), // Repository info card
                    Constraint::Length(1), // Spacer
                    Constraint::Length(3), // Branch input
                    Constraint::Length(1), // Spacer
                    Constraint::Length(1), // Agent selector
                    Constraint::Length(1), // Model selector
                    Constraint::Length(1), // Spacer
                    Constraint::Length(2), // Instructions
                ]
            } else {
                vec![
                    Constraint::Length(6), // Repository info card
                    Constraint::Length(1), // Spacer
                    Constraint::Length(3), // Branch input
                    Constraint::Length(1), // Spacer
                    Constraint::Length(1), // Agent selector
                    Constraint::Length(1), // Spacer
                    Constraint::Length(2), // Instructions
                ]
            })
            .split(inner);

        // Repository info card with icon
        let (repo_name, repo_path) = if let Some(selected_idx) = session_state.selected_repo_index {
            if let Some((_, repo)) = session_state.filtered_repos.get(selected_idx) {
                let name = repo.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                let path = repo.to_string_lossy().to_string();
                // Truncate long paths
                let display_path = if path.len() > 60 {
                    format!("...{}", &path[path.len()-57..])
                } else {
                    path
                };
                (name.to_string(), display_path)
            } else {
                ("Unknown".to_string(), "".to_string())
            }
        } else {
            ("None selected".to_string(), "".to_string())
        };

        let repo_lines = vec![
            Line::from(vec![
                Span::styled("  📁 ", Style::default()),
                Span::styled("Repository", Style::default().fg(Color::Rgb(150, 150, 150))),
            ]),
            Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    &repo_name,
                    Style::default()
                        .fg(Color::Rgb(100, 200, 255)) // Light blue
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  📍 ", Style::default()),
                Span::styled("Path", Style::default().fg(Color::Rgb(150, 150, 150))),
            ]),
            Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    repo_path,
                    Style::default().fg(Color::Rgb(180, 180, 180)).add_modifier(Modifier::ITALIC),
                ),
            ]),
        ];

        let repo_display = Paragraph::new(repo_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(subdued_border))
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
            );
        frame.render_widget(repo_display, chunks[0]);

        // Branch input with icon and cursor
        let branch_text = if session_state.branch_name.is_empty() {
            Line::from(vec![
                Span::styled("  🔀 ", Style::default().fg(selection_green)),
                Span::styled(
                    "ainb/",
                    Style::default().fg(muted_gray).add_modifier(Modifier::ITALIC),
                ),
                Span::styled("█", Style::default().fg(selection_green)),
            ])
        } else {
            Line::from(vec![
                Span::styled("  🔀 ", Style::default().fg(selection_green)),
                Span::styled(
                    &session_state.branch_name,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█", Style::default().fg(selection_green)),
            ])
        };

        let branch_input = Paragraph::new(branch_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(selection_green))
                    .title(Span::styled(
                        " Branch Name ",
                        Style::default().fg(selection_green),
                    ))
                    .style(Style::default().bg(Color::Rgb(35, 35, 45))),
            );
        frame.render_widget(branch_input, chunks[2]);

        // Agent selector bar
        let agent_area = chunks[4];
        self.render_inline_agent_selector(frame, agent_area, session_state, &[
            gold, soft_white, muted_gray, selection_green, list_highlight_bg,
        ]);

        // Model selector bar (only if Claude is selected)
        let instruction_area_idx = if show_model {
            self.render_inline_model_selector(frame, chunks[5], session_state, &[
                gold, soft_white, muted_gray, selection_green, list_highlight_bg,
            ]);
            7 // Instructions at index 7
        } else {
            6 // Instructions at index 6
        };

        // Styled instructions footer
        let mut instruction_spans = vec![
            Span::styled("  ⌨️  ", Style::default()),
            Span::styled("Type", Style::default().fg(selection_green)),
            Span::styled(" branch  ", Style::default().fg(muted_gray)),
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled("  Tab ", Style::default().fg(gold)),
            Span::styled("Agent  ", Style::default().fg(muted_gray)),
        ];

        if show_model {
            instruction_spans.extend(vec![
                Span::styled("│", Style::default().fg(subdued_border)),
                Span::styled("  ←→ ", Style::default().fg(gold)),
                Span::styled("Model  ", Style::default().fg(muted_gray)),
            ]);
        }

        instruction_spans.extend(vec![
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled("  ⏎ ", Style::default().fg(selection_green)),
            Span::styled("Create  ", Style::default().fg(muted_gray)),
            Span::styled("│", Style::default().fg(subdued_border)),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel", Style::default().fg(muted_gray)),
        ]);

        let instructions = Line::from(instruction_spans);
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(instructions_widget, chunks[instruction_area_idx]);
    }

    fn render_permissions_config(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        // Modern color palette
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(120, 120, 140);
        let selection_green = Color::Rgb(100, 200, 100);
        let warning_orange = Color::Rgb(255, 165, 0);

        // Clear background
        let background = Block::default().style(Style::default().bg(dark_bg));
        frame.render_widget(background, area);

        // Main dialog with rounded border
        let title_line = Line::from(vec![
            Span::styled(" 🔐 ", Style::default().fg(gold)),
            Span::styled("Permission Settings", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(title_line)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Length(6), // Description
                Constraint::Length(7), // Option cards
                Constraint::Length(2), // Footer
            ])
            .split(inner);

        // Subtitle
        let subtitle = Paragraph::new(Line::from(vec![
            Span::styled("Configure how Claude handles command execution", Style::default().fg(muted_gray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(subtitle, chunks[0]);

        // Description with info box
        let desc_lines = vec![
            Line::from(vec![
                Span::styled("  ℹ️  ", Style::default().fg(cornflower_blue)),
                Span::styled("About Permission Prompts", Style::default().fg(cornflower_blue).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  • ", Style::default().fg(muted_gray)),
                Span::styled("With prompts: ", Style::default().fg(soft_white)),
                Span::styled("Claude will ask before running commands", Style::default().fg(muted_gray)),
            ]),
            Line::from(vec![
                Span::styled("  • ", Style::default().fg(muted_gray)),
                Span::styled("Without prompts: ", Style::default().fg(soft_white)),
                Span::styled("Claude runs commands immediately (faster)", Style::default().fg(muted_gray)),
            ]),
        ];

        let description = Paragraph::new(desc_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
                    .style(Style::default().bg(dark_bg)),
            );
        frame.render_widget(description, chunks[1]);

        // Options with visual selection
        let (option_icon, option_color, option_title, option_desc, option_flag) = if session_state.skip_permissions {
            (
                "🚀",
                warning_orange,
                "Skip Permission Prompts",
                "Claude will execute commands without asking",
                "--dangerously-skip-permissions",
            )
        } else {
            (
                "🛡️",
                selection_green,
                "Keep Permission Prompts",
                "Claude will ask before executing commands",
                "default",
            )
        };

        let option_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("    {}  ", option_icon), Style::default().fg(option_color)),
                Span::styled(option_title, Style::default().fg(option_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("       ", Style::default()),
                Span::styled(option_desc, Style::default().fg(soft_white)),
            ]),
            Line::from(vec![
                Span::styled("       Flag: ", Style::default().fg(muted_gray)),
                Span::styled(option_flag, Style::default().fg(cornflower_blue).add_modifier(Modifier::ITALIC)),
            ]),
        ];

        let option_title_line = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("Current Selection", Style::default().fg(option_color)),
            Span::styled(" ", Style::default()),
        ]);

        let options = Paragraph::new(option_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(option_color))
                    .title(option_title_line)
                    .style(Style::default().bg(dark_bg)),
            );
        frame.render_widget(options, chunks[2]);

        // Modern footer with keyboard hints
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Space", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" Toggle", Style::default().fg(muted_gray)),
            Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
            Span::styled("Enter", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" Continue", Style::default().fg(muted_gray)),
            Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
            Span::styled("Esc", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" Cancel", Style::default().fg(muted_gray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[3]);
    }

    fn render_creating(&self, frame: &mut Frame, area: Rect) {
        // Modern color palette
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(120, 120, 140);
        let progress_cyan = Color::Rgb(100, 200, 230);

        // Clear background
        let background = Block::default().style(Style::default().bg(dark_bg));
        frame.render_widget(background, area);

        // Main dialog with rounded border
        let title_line = Line::from(vec![
            Span::styled(" ⚙️  ", Style::default().fg(progress_cyan)),
            Span::styled("Creating Session", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(title_line)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Min(0),    // Progress content
                Constraint::Length(2), // Footer
            ])
            .split(inner);

        // Subtitle
        let subtitle = Paragraph::new(Line::from(vec![
            Span::styled("Setting up your development environment...", Style::default().fg(muted_gray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(subtitle, chunks[0]);

        // Progress content with animated-style spinner dots
        let progress_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  🔄 ", Style::default().fg(progress_cyan)),
                Span::styled("Creating Git worktree", Style::default().fg(soft_white)),
                Span::styled(" ...", Style::default().fg(progress_cyan)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  🐳 ", Style::default().fg(cornflower_blue)),
                Span::styled("Initializing Docker container", Style::default().fg(soft_white)),
                Span::styled(" ...", Style::default().fg(cornflower_blue)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  📦 ", Style::default().fg(gold)),
                Span::styled("Mounting volumes and configuring environment", Style::default().fg(soft_white)),
            ]),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("       This may take a moment...", Style::default().fg(muted_gray).add_modifier(Modifier::ITALIC)),
            ]),
        ];

        let progress = Paragraph::new(progress_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
                    .style(Style::default().bg(dark_bg)),
            );
        frame.render_widget(progress, chunks[1]);

        // Modern footer
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("⏳ ", Style::default().fg(progress_cyan)),
            Span::styled("Please wait", Style::default().fg(muted_gray)),
            Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
            Span::styled("Esc", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" Cancel", Style::default().fg(muted_gray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);
    }

    fn render_mode_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        session_state: &NewSessionState,
    ) {
        use crate::models::SessionMode;

        // Draw outer border with modern styling
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237))) // Cornflower blue
            .title(Span::styled(
                " 🎯 Choose Session Mode ",
                Style::default()
                    .fg(Color::Rgb(255, 215, 0)) // Gold
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35))); // Dark background
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(8), // Interactive mode card
                Constraint::Length(1), // Spacer
                Constraint::Length(8), // Boss mode card
                Constraint::Length(1), // Spacer
                Constraint::Length(2), // Instructions
            ])
            .split(inner);

        let is_interactive = session_state.mode == SessionMode::Interactive;
        let is_boss = session_state.mode == SessionMode::Boss;

        // Interactive mode card
        let interactive_border_color = if is_interactive {
            Color::Rgb(100, 200, 100) // Green when selected
        } else {
            Color::Rgb(70, 70, 90) // Gray when not
        };

        let interactive_bg = if is_interactive {
            Color::Rgb(35, 45, 35) // Slightly green tint
        } else {
            Color::Rgb(30, 30, 40)
        };

        let interactive_text = vec![
            Line::from(vec![
                Span::styled(
                    if is_interactive { "  ▶ " } else { "    " },
                    Style::default().fg(Color::Rgb(100, 200, 100)),
                ),
                Span::styled("🖥️  ", Style::default()),
                Span::styled(
                    "Interactive Mode",
                    Style::default()
                        .fg(if is_interactive { Color::Rgb(100, 200, 100) } else { Color::Rgb(200, 200, 200) })
                        .add_modifier(Modifier::BOLD),
                ),
                if is_interactive {
                    Span::styled("  ✓", Style::default().fg(Color::Rgb(100, 200, 100)))
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(100, 149, 237))),
                Span::styled(" Traditional development with shell access", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(100, 149, 237))),
                Span::styled(" Full Claude CLI features and MCP servers", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(100, 149, 237))),
                Span::styled(" Attach to container for development", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
        ];

        let interactive_para = Paragraph::new(interactive_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(interactive_border_color))
                    .style(Style::default().bg(interactive_bg)),
            );
        frame.render_widget(interactive_para, chunks[0]);

        // Boss mode card
        let boss_border_color = if is_boss {
            Color::Rgb(255, 165, 0) // Orange when selected
        } else {
            Color::Rgb(70, 70, 90) // Gray when not
        };

        let boss_bg = if is_boss {
            Color::Rgb(45, 40, 30) // Slightly orange tint
        } else {
            Color::Rgb(30, 30, 40)
        };

        let boss_text = vec![
            Line::from(vec![
                Span::styled(
                    if is_boss { "  ▶ " } else { "    " },
                    Style::default().fg(Color::Rgb(255, 165, 0)),
                ),
                Span::styled("🤖 ", Style::default()),
                Span::styled(
                    "Boss Mode",
                    Style::default()
                        .fg(if is_boss { Color::Rgb(255, 165, 0) } else { Color::Rgb(200, 200, 200) })
                        .add_modifier(Modifier::BOLD),
                ),
                if is_boss {
                    Span::styled("  ✓", Style::default().fg(Color::Rgb(255, 165, 0)))
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::styled(" Non-interactive task execution", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::styled(" Direct prompt execution with text output", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("•", Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::styled(" Results streamed to TUI logs", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
        ];

        let boss_para = Paragraph::new(boss_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(boss_border_color))
                    .style(Style::default().bg(boss_bg)),
            );
        frame.render_widget(boss_para, chunks[2]);

        // Styled instructions footer
        let instructions = Line::from(vec![
            Span::styled("  ↑↓ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Switch Mode  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  ⏎ ", Style::default().fg(Color::Rgb(100, 200, 100))),
            Span::styled("Continue  ", Style::default().fg(Color::Rgb(128, 128, 128))),
            Span::styled("│", Style::default().fg(Color::Rgb(70, 70, 90))),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(255, 100, 100))),
            Span::styled("Cancel  ", Style::default().fg(Color::Rgb(128, 128, 128))),
        ]);

        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)));
        frame.render_widget(instructions_widget, chunks[4]);
    }

    fn render_prompt_input(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        // Modern color palette
        let cornflower_blue = Color::Rgb(100, 149, 237);
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(120, 120, 140);
        let file_finder_yellow = Color::Rgb(255, 200, 100);

        // Clear background
        let background = Block::default().style(Style::default().bg(dark_bg));
        frame.render_widget(background, area);

        // Main dialog with rounded border
        let title_line = Line::from(vec![
            Span::styled(" 💬 ", Style::default().fg(gold)),
            Span::styled("Task Prompt", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cornflower_blue))
            .title(title_line)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(dark_bg));
        frame.render_widget(block.clone(), area);

        // Inner area for content
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Length(6), // Instructions
                Constraint::Min(0),    // Prompt input area
                Constraint::Length(2), // Controls
            ])
            .split(inner);

        // Subtitle
        let subtitle_text = if session_state.file_finder.is_active {
            "File finder active - search for files to reference"
        } else {
            "Enter the task or prompt for Claude to execute"
        };
        let subtitle = Paragraph::new(Line::from(vec![
            Span::styled(subtitle_text, Style::default().fg(muted_gray)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(subtitle, chunks[0]);

        // Instructions - update to mention @ symbol for file finder
        let instructions_lines = if session_state.file_finder.is_active {
            vec![
                Line::from(vec![
                    Span::styled("  🔍 ", Style::default().fg(file_finder_yellow)),
                    Span::styled("File Finder Active", Style::default().fg(file_finder_yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(muted_gray)),
                    Span::styled("↑/↓", Style::default().fg(gold)),
                    Span::styled(" to navigate files", Style::default().fg(soft_white)),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(muted_gray)),
                    Span::styled("Enter", Style::default().fg(gold)),
                    Span::styled(" to select file  •  ", Style::default().fg(soft_white)),
                    Span::styled("Esc", Style::default().fg(gold)),
                    Span::styled(" to cancel", Style::default().fg(soft_white)),
                ]),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled("  💡 ", Style::default().fg(cornflower_blue)),
                    Span::styled("Example prompts:", Style::default().fg(cornflower_blue)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(muted_gray)),
                    Span::styled("\"Analyze this codebase and suggest improvements\"", Style::default().fg(soft_white).add_modifier(Modifier::ITALIC)),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(muted_gray)),
                    Span::styled("\"Review the file ", Style::default().fg(soft_white).add_modifier(Modifier::ITALIC)),
                    Span::styled("@src/main.rs", Style::default().fg(file_finder_yellow)),
                    Span::styled("\" (type @ for file finder)", Style::default().fg(muted_gray)),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(muted_gray)),
                    Span::styled("\"Fix issue #123\"", Style::default().fg(soft_white).add_modifier(Modifier::ITALIC)),
                ]),
            ]
        };

        let instructions_border = if session_state.file_finder.is_active {
            file_finder_yellow
        } else {
            Color::Rgb(60, 60, 80)
        };

        let instructions = Paragraph::new(instructions_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(instructions_border))
                    .style(Style::default().bg(dark_bg)),
            );
        frame.render_widget(instructions, chunks[1]);

        // Split the prompt input area if file finder is active
        if session_state.file_finder.is_active {
            let input_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50), // Prompt
                    Constraint::Percentage(50), // File finder
                ])
                .split(chunks[2]);

            // Render prompt on the left
            self.render_text_editor(frame, input_chunks[0], &session_state.boss_prompt, "Prompt");

            // Render file finder on the right
            self.render_file_finder(frame, input_chunks[1], session_state);
        } else {
            // Normal full-width prompt input
            self.render_text_editor(frame, chunks[2], &session_state.boss_prompt, "Prompt");
        }

        // Modern footer with keyboard hints
        let controls = if session_state.file_finder.is_active {
            Paragraph::new(Line::from(vec![
                Span::styled("↑↓", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Navigate", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("Enter", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Select", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("Type", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Filter", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("Esc", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(muted_gray)),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled("Type", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Input", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("Ctrl+J", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Newline", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("@", Style::default().fg(file_finder_yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" Files", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("Enter", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Continue", Style::default().fg(muted_gray)),
                Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 80))),
                Span::styled("Esc", Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(muted_gray)),
            ]))
        };
        frame.render_widget(controls.alignment(Alignment::Center), chunks[3]);
    }

    fn render_file_finder(&self, frame: &mut Frame, area: Rect, session_state: &NewSessionState) {
        // Modern color palette
        let dark_bg = Color::Rgb(25, 25, 35);
        let gold = Color::Rgb(255, 215, 0);
        let soft_white = Color::Rgb(220, 220, 230);
        let muted_gray = Color::Rgb(120, 120, 140);
        let file_finder_yellow = Color::Rgb(255, 200, 100);
        let selection_bg = Color::Rgb(80, 70, 40);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Query input
                Constraint::Min(0),    // File list
            ])
            .split(area);

        // Query input with modern styling
        let query_display = format!("  @{}", session_state.file_finder.query);
        let query_title = Line::from(vec![
            Span::styled(" 🔍 ", Style::default().fg(file_finder_yellow)),
            Span::styled("Filter", Style::default().fg(file_finder_yellow)),
            Span::styled(" ", Style::default()),
        ]);

        let query_input = Paragraph::new(Line::from(vec![
            Span::styled(query_display, Style::default().fg(file_finder_yellow)),
            Span::styled("▋", Style::default().fg(gold)), // Cursor indicator
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(file_finder_yellow))
                .title(query_title)
                .style(Style::default().bg(dark_bg)),
        );
        frame.render_widget(query_input, chunks[0]);

        // File list with modern styling
        let file_items: Vec<ListItem> = session_state
            .file_finder
            .matches
            .iter()
            .enumerate()
            .map(|(idx, file_match)| {
                if idx == session_state.file_finder.selected_index {
                    ListItem::new(Line::from(vec![
                        Span::styled("  ▶ ", Style::default().fg(gold)),
                        Span::styled(&file_match.relative_path, Style::default().fg(gold).add_modifier(Modifier::BOLD)),
                    ]))
                    .style(Style::default().bg(selection_bg))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled("    ", Style::default()),
                        Span::styled(&file_match.relative_path, Style::default().fg(soft_white)),
                    ]))
                }
            })
            .collect();

        let match_count = session_state.file_finder.matches.len();
        let list_title = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("📄 ", Style::default().fg(muted_gray)),
            Span::styled(format!("{} matches", match_count), Style::default().fg(muted_gray)),
            Span::styled(" ", Style::default()),
        ]);

        let file_list = List::new(file_items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
                .title(list_title)
                .style(Style::default().bg(dark_bg)),
        );

        frame.render_widget(file_list, chunks[1]);
    }

    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

    fn render_text_editor(
        &self,
        frame: &mut Frame,
        area: Rect,
        editor: &crate::app::state::TextEditor,
        title: &str,
    ) {
        use ratatui::layout::Alignment;
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(title);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if editor.is_empty() {
            // Show placeholder text
            let placeholder = Paragraph::new("Type your prompt here...")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Left);
            frame.render_widget(placeholder, inner_area);
        } else {
            // Render text with cursor
            let (cursor_line, cursor_col) = editor.get_cursor_position();
            let lines = editor.get_lines();

            let rendered_lines: Vec<Line> = lines
                .iter()
                .enumerate()
                .map(|(line_idx, line_text)| {
                    if line_idx == cursor_line {
                        // This line contains the cursor
                        let mut spans = Vec::new();

                        if cursor_col == 0 {
                            // Cursor at beginning of line
                            spans.push(Span::styled(
                                "█",
                                Style::default().fg(Color::White).bg(Color::Green),
                            ));
                            if !line_text.is_empty() {
                                spans.push(Span::styled(
                                    line_text,
                                    Style::default().fg(Color::White),
                                ));
                            }
                        } else if cursor_col >= line_text.len() {
                            // Cursor at end of line
                            spans.push(Span::styled(line_text, Style::default().fg(Color::White)));
                            spans.push(Span::styled(
                                "█",
                                Style::default().fg(Color::White).bg(Color::Green),
                            ));
                        } else {
                            // Cursor in middle of line
                            let (before, rest) = line_text.split_at(cursor_col);
                            let (cursor_char, after) = if rest.len() > 1 {
                                rest.split_at(1)
                            } else {
                                (rest, "")
                            };

                            if !before.is_empty() {
                                spans.push(Span::styled(before, Style::default().fg(Color::White)));
                            }
                            spans.push(Span::styled(
                                cursor_char,
                                Style::default().fg(Color::White).bg(Color::Green),
                            ));
                            if !after.is_empty() {
                                spans.push(Span::styled(after, Style::default().fg(Color::White)));
                            }
                        }

                        Line::from(spans)
                    } else {
                        // Normal line without cursor
                        Line::from(Span::styled(line_text, Style::default().fg(Color::White)))
                    }
                })
                .collect();

            let paragraph = Paragraph::new(rendered_lines)
                .alignment(Alignment::Left)
                .wrap(ratatui::widgets::Wrap { trim: false }); // Don't trim to preserve exact formatting

            frame.render_widget(paragraph, inner_area);
        }
    }
}

impl Default for NewSessionComponent {
    fn default() -> Self {
        Self::new()
    }
}
