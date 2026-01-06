// ABOUTME: Welcome panel component displaying getting started info and architecture overview
// Replaces the duplicate action cards on the home screen right side

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

// Color palette from TUI style guide
const CORNFLOWER_BLUE: Color = Color::Rgb(100, 149, 237);
const GOLD: Color = Color::Rgb(255, 215, 0);
const SELECTION_GREEN: Color = Color::Rgb(100, 200, 100);
const DARK_BG: Color = Color::Rgb(25, 25, 35);
const PANEL_BG: Color = Color::Rgb(30, 30, 40);
const SOFT_WHITE: Color = Color::Rgb(220, 220, 230);
const MUTED_GRAY: Color = Color::Rgb(120, 120, 140);
const SUBDUED_BORDER: Color = Color::Rgb(60, 60, 80);
const ACCENT_CYAN: Color = Color::Rgb(80, 200, 220);

/// Welcome panel state
#[derive(Debug, Default)]
pub struct WelcomePanelState {
    /// Whether the panel is focused (not typically focused)
    pub is_focused: bool,
}

impl WelcomePanelState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Welcome panel component
pub struct WelcomePanelComponent;

impl WelcomePanelComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, _state: &WelcomePanelState) {
        // Main container block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBDUED_BORDER))
            .style(Style::default().bg(PANEL_BG));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Layout sections
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),  // Title + tagline
                Constraint::Length(1),  // Spacer
                Constraint::Length(9),  // Quick start steps
                Constraint::Length(1),  // Spacer
                Constraint::Min(8),     // Architecture diagram
                Constraint::Length(3),  // Tip/footer
            ])
            .split(inner);

        self.render_header(frame, layout[0]);
        self.render_quick_start(frame, layout[2]);
        self.render_architecture(frame, layout[4]);
        self.render_tip(frame, layout[5]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header_lines = vec![
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("Welcome to ", Style::default().fg(SOFT_WHITE)),
                Span::styled("AINB", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "AI-powered development environment manager",
                    Style::default().fg(MUTED_GRAY).add_modifier(Modifier::ITALIC),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "for Claude Code agents in isolated workspaces",
                    Style::default().fg(MUTED_GRAY).add_modifier(Modifier::ITALIC),
                ),
            ]),
        ];

        let header = Paragraph::new(header_lines)
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(header, area);
    }

    fn render_quick_start(&self, frame: &mut Frame, area: Rect) {
        let steps = vec![
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("Quick Start", Style::default().fg(ACCENT_CYAN).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("1.", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" Select an ", Style::default().fg(SOFT_WHITE)),
                Span::styled("Agent", Style::default().fg(CORNFLOWER_BLUE)),
                Span::styled(" to configure your Claude instance", Style::default().fg(SOFT_WHITE)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("2.", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" Browse ", Style::default().fg(SOFT_WHITE)),
                Span::styled("Catalog", Style::default().fg(CORNFLOWER_BLUE)),
                Span::styled(" for project templates", Style::default().fg(SOFT_WHITE)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("3.", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(" Launch ", Style::default().fg(SOFT_WHITE)),
                Span::styled("Sessions", Style::default().fg(CORNFLOWER_BLUE)),
                Span::styled(" to start working", Style::default().fg(SOFT_WHITE)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("Press ", Style::default().fg(MUTED_GRAY)),
                Span::styled("Enter", Style::default().fg(SELECTION_GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" to activate selection, ", Style::default().fg(MUTED_GRAY)),
                Span::styled("↑↓", Style::default().fg(SELECTION_GREEN)),
                Span::styled(" to navigate", Style::default().fg(MUTED_GRAY)),
            ]),
        ];

        let paragraph = Paragraph::new(steps)
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(paragraph, area);
    }

    fn render_architecture(&self, frame: &mut Frame, area: Rect) {
        // ASCII architecture diagram
        let diagram = vec![
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("Architecture", Style::default().fg(ACCENT_CYAN).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("┌─────────┐", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     ┌──────────┐", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("  AINB  ", Style::default().fg(GOLD)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("────▶│", Style::default().fg(CORNFLOWER_BLUE)),
                Span::styled(" Worktree ", Style::default().fg(SOFT_WHITE)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("   TUI  ", Style::default().fg(GOLD)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     │", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("  + tmux  ", Style::default().fg(SOFT_WHITE)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("└─────────┘", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     └──────────┘", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("       │", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("              │", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("       ▼", Style::default().fg(CORNFLOWER_BLUE)),
                Span::styled("              ▼", Style::default().fg(CORNFLOWER_BLUE)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("┌─────────┐", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     ┌──────────┐", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled(" Config  ", Style::default().fg(SELECTION_GREEN)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     │", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("  Claude  ", Style::default().fg(ACCENT_CYAN)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled(" Presets ", Style::default().fg(SELECTION_GREEN)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     │", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("   Code   ", Style::default().fg(ACCENT_CYAN)),
                Span::styled("│", Style::default().fg(SUBDUED_BORDER)),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("└─────────┘", Style::default().fg(SUBDUED_BORDER)),
                Span::styled("     └──────────┘", Style::default().fg(SUBDUED_BORDER)),
            ]),
        ];

        let paragraph = Paragraph::new(diagram)
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(paragraph, area);
    }

    fn render_tip(&self, frame: &mut Frame, area: Rect) {
        let tip = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("💡 ", Style::default()),
                Span::styled("Tip: ", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
                Span::styled(
                    "Each session runs in an isolated git worktree",
                    Style::default().fg(MUTED_GRAY),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(tip)
            .style(Style::default().bg(PANEL_BG))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }
}

impl Default for WelcomePanelComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_panel_state() {
        let state = WelcomePanelState::new();
        assert!(!state.is_focused);
    }
}
