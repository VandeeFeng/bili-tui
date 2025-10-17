use crate::app::{App, Focusable, InputMode, MessageLevel};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
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

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),     // Search bar
            Constraint::Min(0),        // Main content area
            Constraint::Length(3),     // Message bar
        ])
        .split(f.size());

    let search_focused = app.focused_panel == Focusable::Search;
    let search_bar = Paragraph::new(app.search_input.value())
        .block(app.create_focused_block("Search", search_focused));
    f.render_widget(search_bar, chunks[0]);

    if app.is_editing() {
        f.set_cursor(
            chunks[0].x + app.search_input.visual_cursor() as u16 + 1,
            chunks[0].y + 1,
        );
    }

    if app.help_active {
        let help_text = vec![
            Line::from("Commands:".bold()),
            Line::from("  video <url>        - Play video with mpv"),
            Line::from("  video-info <url>   - Show video details"),
            Line::from("  help               - Show this help message"),
            Line::from("  q                  - Quit the application"),
            Line::from(""),
            Line::from("Navigation:".bold()),
            Line::from("  j/k                - Move focus between panels"),
            Line::from("  Enter              - Select/Enter panel"),
            Line::from("  q/Esc              - Exit current mode/panel"),
            Line::from("  :                  - Open command popup"),
            Line::from("  ?                  - Show help"),
        ];
        let help_panel = Paragraph::new(help_text)
            .block(Block::default().title("Help").borders(Borders::ALL));
        f.render_widget(help_panel, chunks[1]);
    } else {
        match app.mode {
            InputMode::Detail => {
                let detail_text = if let Some(info) = &app.video_info {
                    vec![
                        Line::from(vec!["Title: ".bold(), Span::raw(info.title.clone())]),
                        Line::from(vec!["Author: ".bold(), Span::raw(info.owner.name.clone())]),
                        Line::from(vec!["Plays: ".bold(), Span::raw(info.stat.view.to_string())]),
                        Line::from(vec!["Likes: ".bold(), Span::raw(info.stat.like.to_string())]),
                        Line::from(""),
                        Line::from(Span::raw(info.desc.clone())),
                        Line::from(""),
                        Line::from("[P]lay with mpv".bold()),
                    ]
                } else if let Some(selected) = app.results_list_state.selected() {
                    if let Some(video) = app.search_results.get(selected) {
                        let text = vec![
                            Line::from(vec![
                                "Title: ".bold(),
                                Span::raw(video.title.clone()),
                            ]),
                            Line::from(vec![
                                "Plays: ".bold(),
                                Span::raw(video.play.to_string().trim_matches('"').to_string()),
                            ]),
                            Line::from(vec![
                                "Likes: ".bold(),
                                Span::raw(video.like.to_string()),
                            ]),
                            Line::from(vec![
                                "Duration: ".bold(),
                                Span::raw(video.duration.clone()),
                            ]),
                            Line::from(""),
                            Line::from(Span::raw(video.description.clone())),
                            Line::from(""),
                            Line::from("[P]lay with mpv".bold()),
                        ];
                        text
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };
                let details_focused = app.focused_panel == Focusable::Results;
                let info_panel = Paragraph::new(detail_text)
                    .wrap(ratatui::widgets::Wrap { trim: true })
                    .block(app.create_focused_block("Video Details", details_focused));
                f.render_widget(info_panel, chunks[1]);
            }
                      _ => {
                let results: Vec<ListItem> = app
                    .search_results
                    .iter()
                    .map(|video| {
                        let text_width = chunks[1].width.saturating_sub(6) as usize;
                        let options = textwrap::Options::new(text_width)
                            .initial_indent("")
                            .subsequent_indent("  ");

                        let title_wrapped = textwrap::wrap(&video.title, options);

                        let mut lines: Vec<Line> = title_wrapped
                            .iter()
                            .map(|s| Line::from(s.to_string()))
                            .collect();

                        let meta_info = format!(
                            "{} (▶ {}) [{}]",
                            video.author,
                            video.play.to_string().trim_matches('"'),
                            video.duration
                        );
                        lines.push(Line::from(meta_info.italic().fg(Color::DarkGray)));
                        lines.push(Line::from(""));

                        ListItem::new(lines)
                    })
                    .collect();

                let results_focused = app.focused_panel == Focusable::Results;
                let results_list = List::new(results)
                    .block(app.create_focused_block("Results", results_focused))
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                    .highlight_symbol(">> ");

                f.render_stateful_widget(results_list, chunks[1], &mut app.results_list_state);
            }
        }
    }

    // Render command popup if active
    if app.is_commanding() {
        let popup_area = app.calculate_popup_area(f.size(), 60, 3);

        // Clear the background area to create a clean background
        f.render_widget(Clear, popup_area);

        let command_popup = Paragraph::new(app.command_input.value())
            .block(app.create_popup_block("Command", Color::Green));
        f.render_widget(command_popup, popup_area);

        f.set_cursor(
            popup_area.x + app.command_input.visual_cursor() as u16 + 1,
            popup_area.y + 1,
        );
    }

    // Render message bar using the new trait
    app.render_message_bar(f, chunks[2], app);

    // Render error popup if there's an error and show_error_popup is true
    if app.show_error_popup
        && let Some(error) = &app.last_error {
            let popup_area = app.calculate_popup_area(f.size(), 60, 3);

            let error_popup = Paragraph::new(error.as_str())
                .block(app.create_popup_block("Error", Color::Red));
            f.render_widget(error_popup, popup_area);
        }
}
