// ABOUTME: Confirmation dialog component for displaying yes/no prompts with keyboard navigation

use crate::app::state::AppState;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub struct ConfirmationDialogComponent;

impl ConfirmationDialogComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        if let Some(dialog) = &state.confirmation_dialog {
            // Clear the area first
            frame.render_widget(Clear, area);

            // Calculate dialog size (center it)
            let dialog_width = 60.min(area.width - 4);
            let dialog_height = 8;

            let dialog_area = Rect {
                x: (area.width - dialog_width) / 2,
                y: (area.height - dialog_height) / 2,
                width: dialog_width,
                height: dialog_height,
            };

            // Render dialog background
            let block = Block::default()
                .title(dialog.title.clone())
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black));

            frame.render_widget(block, dialog_area);

            // Create inner layout
            let inner_area = Rect {
                x: dialog_area.x + 1,
                y: dialog_area.y + 1,
                width: dialog_area.width - 2,
                height: dialog_area.height - 2,
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),    // Message
                    Constraint::Length(2), // Buttons
                ])
                .split(inner_area);

            // Render message
            let message = Paragraph::new(dialog.message.clone())
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::White));

            frame.render_widget(message, chunks[0]);

            // Render buttons
            let button_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            // Yes button
            let yes_style = if dialog.selected_option {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            let yes_button = Paragraph::new("Yes").style(yes_style).alignment(Alignment::Center);

            frame.render_widget(yes_button, button_chunks[0]);

            // No button
            let no_style = if !dialog.selected_option {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            let no_button = Paragraph::new("No").style(no_style).alignment(Alignment::Center);

            frame.render_widget(no_button, button_chunks[1]);
        }
    }
}
