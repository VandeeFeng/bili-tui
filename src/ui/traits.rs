use crate::app::{App, MessageLevel};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// Trait for rendering UI widgets with consistent styling
pub trait WidgetRenderer {
    /// Create a bordered block with focus-aware styling
    fn create_focused_block<'a>(&self, title: &'a str, focused: bool) -> Block<'a>;

    /// Create a popup block with consistent styling
    fn create_popup_block<'a>(&self, title: &'a str, color: Color) -> Block<'a>;

    /// Calculate popup area centered in the given frame
    fn calculate_popup_area(&self, frame_size: Rect, width: u16, height: u16) -> Rect;
}

/// Trait for rendering messages with consistent styling
pub trait MessageRenderer {
    /// Get the color and style for a message level
    fn get_message_style(&self, level: &MessageLevel) -> Style;

    /// Render a message bar with the latest message
    fn render_message_bar(&self, f: &mut Frame, area: Rect, app: &App);
}

impl WidgetRenderer for App {
    fn create_focused_block<'a>(&self, title: &'a str, focused: bool) -> Block<'a> {
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if focused {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            })
    }

    fn create_popup_block<'a>(&self, title: &'a str, color: Color) -> Block<'a> {
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
    }

    fn calculate_popup_area(&self, frame_size: Rect, width: u16, height: u16) -> Rect {
        let popup_width = width.min(frame_size.width.saturating_sub(4));
        let popup_x = (frame_size.width - popup_width) / 2;
        let popup_y = (frame_size.height - height) / 2;
        Rect::new(popup_x, popup_y, popup_width, height)
    }
}

impl MessageRenderer for App {
    fn get_message_style(&self, level: &MessageLevel) -> Style {
        match level {
            MessageLevel::Info => Style::default().fg(Color::Blue),
            MessageLevel::Success => Style::default().fg(Color::Green),
            MessageLevel::Warning => Style::default().fg(Color::Yellow),
            MessageLevel::Error => Style::default().fg(Color::Red),
        }
    }

    fn render_message_bar(&self, f: &mut Frame, area: Rect, app: &App) {
        if let Some(message) = app.get_latest_message() {
            let style = self.get_message_style(&message.level);
            let message_bar = Paragraph::new(message.text.clone())
                .block(
                    Block::default()
                        .title("Messages")
                        .borders(Borders::ALL)
                        .border_style(style),
                );
            f.render_widget(message_bar, area);
        } else {
            // Render empty message bar
            let empty_message_bar = Paragraph::new("")
                .block(
                    Block::default()
                        .title("Messages")
                        .borders(Borders::ALL),
                );
            f.render_widget(empty_message_bar, area);
        }
    }
}