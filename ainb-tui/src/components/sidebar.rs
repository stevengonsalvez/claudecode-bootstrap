// ABOUTME: Sidebar navigation component for AINB home screen
// Inspired by VS Code, Discord, and Slack sidebar patterns

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

// Color palette from TUI style guide
const CORNFLOWER_BLUE: Color = Color::Rgb(100, 149, 237);
const GOLD: Color = Color::Rgb(255, 215, 0);
const SELECTION_GREEN: Color = Color::Rgb(100, 200, 100);
const DARK_BG: Color = Color::Rgb(25, 25, 35);
const PANEL_BG: Color = Color::Rgb(30, 30, 40);
const LIST_HIGHLIGHT_BG: Color = Color::Rgb(40, 40, 60);
const SOFT_WHITE: Color = Color::Rgb(220, 220, 230);
const MUTED_GRAY: Color = Color::Rgb(120, 120, 140);
const SUBDUED_BORDER: Color = Color::Rgb(60, 60, 80);

/// Sidebar navigation items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    // Main section
    Home,
    // Sessions section
    NewAgent,
    ActiveSessions,
    History,
    // Tools section
    Git,
    Catalog,
    Config,
    // System section
    Help,
    Quit,
}

impl SidebarItem {
    /// Get the display icon for this item
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Home => "",
            Self::NewAgent => "",
            Self::ActiveSessions => "",
            Self::History => "",
            Self::Git => "",
            Self::Catalog => "",
            Self::Config => "",
            Self::Help => "",
            Self::Quit => "",
        }
    }

    /// Get the display label for this item
    pub fn label(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::NewAgent => "New Agent",
            Self::ActiveSessions => "Sessions",
            Self::History => "History",
            Self::Git => "Git",
            Self::Catalog => "Catalog",
            Self::Config => "Config",
            Self::Help => "Help",
            Self::Quit => "Quit",
        }
    }

    /// Get the keyboard shortcut for this item
    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Home => "q",
            Self::NewAgent => "n",
            Self::ActiveSessions => "s",
            Self::History => "h",
            Self::Git => "g",
            Self::Catalog => "c",
            Self::Config => "C",
            Self::Help => "?",
            Self::Quit => "Q",
        }
    }

    /// Get the section this item belongs to
    pub fn section(&self) -> SidebarSection {
        match self {
            Self::Home => SidebarSection::Main,
            Self::NewAgent | Self::ActiveSessions | Self::History => SidebarSection::Sessions,
            Self::Git | Self::Catalog | Self::Config => SidebarSection::Tools,
            Self::Help | Self::Quit => SidebarSection::System,
        }
    }

    /// Get all items in order
    pub fn all() -> &'static [SidebarItem] {
        &[
            Self::Home,
            Self::NewAgent,
            Self::ActiveSessions,
            Self::History,
            Self::Git,
            Self::Catalog,
            Self::Config,
            Self::Help,
            Self::Quit,
        ]
    }
}

/// Sidebar sections for grouping items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    Main,
    Sessions,
    Tools,
    System,
}

impl SidebarSection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Main => "",
            Self::Sessions => "Sessions",
            Self::Tools => "Tools",
            Self::System => "",
        }
    }
}

/// Sidebar state
#[derive(Debug)]
pub struct SidebarState {
    /// Currently selected item index
    pub selected_index: usize,
    /// Whether the sidebar is focused
    pub is_focused: bool,
    /// Whether to show labels (false = icon-only mode)
    pub show_labels: bool,
    /// Active sessions count (for badge display)
    pub active_sessions_count: usize,
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            is_focused: true,
            show_labels: true,
            active_sessions_count: 0,
        }
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> SidebarItem {
        SidebarItem::all()[self.selected_index]
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let max_index = SidebarItem::all().len() - 1;
        if self.selected_index < max_index {
            self.selected_index += 1;
        }
    }

    /// Set selection to a specific item
    pub fn select(&mut self, item: SidebarItem) {
        if let Some(index) = SidebarItem::all().iter().position(|&i| i == item) {
            self.selected_index = index;
        }
    }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self::new()
    }
}

/// Sidebar component for rendering
pub struct SidebarComponent;

impl SidebarComponent {
    pub fn new() -> Self {
        Self
    }

    /// Render the sidebar
    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SidebarState) {
        // Outer block with border
        let border_color = if state.is_focused {
            CORNFLOWER_BLUE
        } else {
            SUBDUED_BORDER
        };

        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(DARK_BG));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Calculate layout for sections
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Top padding
                Constraint::Length(2),  // Home (main section)
                Constraint::Length(1),  // Separator
                Constraint::Length(1),  // Sessions header
                Constraint::Length(4),  // Session items (3 items + spacing)
                Constraint::Length(1),  // Separator
                Constraint::Length(1),  // Tools header
                Constraint::Length(4),  // Tool items (3 items + spacing)
                Constraint::Length(1),  // Separator
                Constraint::Min(0),     // Flexible space
                Constraint::Length(3),  // System items (Help, Quit)
            ])
            .split(inner);

        let items = SidebarItem::all();
        let mut current_section = SidebarSection::Main;

        // Render Home item
        self.render_item(frame, layout[1], &items[0], state.selected_index == 0, state);

        // Sessions section header
        self.render_section_header(frame, layout[3], "Sessions");

        // Session items (indices 1-3)
        let session_area = layout[4];
        let session_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(session_area);

        for (i, item_idx) in (1..=3).enumerate() {
            let is_selected = state.selected_index == item_idx;
            let badge = if items[item_idx] == SidebarItem::ActiveSessions && state.active_sessions_count > 0 {
                Some(state.active_sessions_count)
            } else {
                None
            };
            self.render_item_with_badge(frame, session_layout[i], &items[item_idx], is_selected, state, badge);
        }

        // Tools section header
        self.render_section_header(frame, layout[6], "Tools");

        // Tool items (indices 4-6)
        let tools_area = layout[7];
        let tools_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(tools_area);

        for (i, item_idx) in (4..=6).enumerate() {
            self.render_item(frame, tools_layout[i], &items[item_idx], state.selected_index == item_idx, state);
        }

        // System items at bottom (indices 7-8)
        let system_area = layout[10];
        let system_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(system_area);

        for (i, item_idx) in (7..=8).enumerate() {
            self.render_item(frame, system_layout[i], &items[item_idx], state.selected_index == item_idx, state);
        }
    }

    fn render_section_header(&self, frame: &mut Frame, area: Rect, label: &str) {
        let header = Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(label.to_uppercase(), Style::default().fg(MUTED_GRAY).add_modifier(Modifier::DIM)),
        ]))
        .style(Style::default().bg(DARK_BG));

        frame.render_widget(header, area);
    }

    fn render_item(&self, frame: &mut Frame, area: Rect, item: &SidebarItem, is_selected: bool, state: &SidebarState) {
        self.render_item_with_badge(frame, area, item, is_selected, state, None);
    }

    fn render_item_with_badge(
        &self,
        frame: &mut Frame,
        area: Rect,
        item: &SidebarItem,
        is_selected: bool,
        state: &SidebarState,
        badge: Option<usize>,
    ) {
        let (indicator, icon_style, label_style, bg_style) = if is_selected && state.is_focused {
            (
                Span::styled("", Style::default().fg(SELECTION_GREEN)),
                Style::default().fg(GOLD),
                Style::default().fg(SOFT_WHITE).add_modifier(Modifier::BOLD),
                Style::default().bg(LIST_HIGHLIGHT_BG),
            )
        } else if is_selected {
            (
                Span::styled(" ", Style::default()),
                Style::default().fg(GOLD),
                Style::default().fg(SOFT_WHITE),
                Style::default().bg(LIST_HIGHLIGHT_BG),
            )
        } else {
            (
                Span::styled(" ", Style::default()),
                Style::default().fg(MUTED_GRAY),
                Style::default().fg(MUTED_GRAY),
                Style::default().bg(DARK_BG),
            )
        };

        let mut spans = vec![
            indicator,
            Span::styled(" ", Style::default()),
            Span::styled(item.icon(), icon_style),
        ];

        if state.show_labels {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(item.label(), label_style));

            // Add badge if present
            if let Some(count) = badge {
                spans.push(Span::styled(" (", Style::default().fg(MUTED_GRAY)));
                spans.push(Span::styled(count.to_string(), Style::default().fg(SELECTION_GREEN)));
                spans.push(Span::styled(")", Style::default().fg(MUTED_GRAY)));
            }
        }

        let line = Paragraph::new(Line::from(spans)).style(bg_style);

        frame.render_widget(line, area);
    }

    /// Get the recommended width for the sidebar
    pub fn recommended_width(state: &SidebarState) -> u16 {
        if state.show_labels {
            20 // With labels
        } else {
            4 // Icons only
        }
    }
}

impl Default for SidebarComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_state_navigation() {
        let mut state = SidebarState::new();
        assert_eq!(state.selected_index, 0);

        state.move_down();
        assert_eq!(state.selected_index, 1);

        state.move_up();
        assert_eq!(state.selected_index, 0);

        // Should not go below 0
        state.move_up();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_sidebar_item_properties() {
        let item = SidebarItem::NewAgent;
        assert_eq!(item.label(), "New Agent");
        assert_eq!(item.shortcut(), "n");
        assert_eq!(item.section(), SidebarSection::Sessions);
    }

    #[test]
    fn test_select_specific_item() {
        let mut state = SidebarState::new();
        state.select(SidebarItem::Git);
        assert_eq!(state.selected_item(), SidebarItem::Git);
    }
}
