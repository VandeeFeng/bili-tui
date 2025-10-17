use crate::app::{App, Focusable, InputMode, MessageLevel};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use textwrap;

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),     // Search bar
            Constraint::Min(0),        // Main content area
            Constraint::Length(3),     // Message bar
        ])
        .split(f.size());

    let search_bar = Paragraph::new(app.search_input.value()).block(
        Block::default()
            .title("Search")
            .borders(Borders::ALL)
            .border_style(if app.focused_panel == Focusable::Search {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            }),
    );
    f.render_widget(search_bar, chunks[0]);

    if app.is_editing() {
        f.set_cursor(
            chunks[0].x + app.search_input.visual_cursor() as u16 + 1,
            chunks[0].y + 1,
        );
    }

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
            let info_panel = Paragraph::new(detail_text)
                .wrap(ratatui::widgets::Wrap { trim: true })
                .block(
                    Block::default()
                        .title("Video Details")
                        .borders(Borders::ALL)
                        .border_style(if app.focused_panel == Focusable::Results {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default()
                        }),
                );
            f.render_widget(info_panel, chunks[1]);
        }
        InputMode::Help => {
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
            ];
            let help_panel = Paragraph::new(help_text)
                .block(Block::default().title("Help").borders(Borders::ALL));
            f.render_widget(help_panel, chunks[1]);
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
                        "{} (▶ {})",
                        video.author,
                        video.play.to_string().trim_matches('"')
                    );
                    lines.push(Line::from(meta_info.italic().fg(Color::DarkGray)));
                    lines.push(Line::from(""));

                    ListItem::new(lines)
                })
                .collect();

            let results_list = List::new(results)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Results")
                        .border_style(if app.focused_panel == Focusable::Results {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default()
                        }),
                )
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">> ");

            f.render_stateful_widget(results_list, chunks[1], &mut app.results_list_state);
        }
    }

    // Render command popup if active
    if app.is_commanding() {
        let popup_width = 60.min(f.size().width.saturating_sub(4));
        let popup_height = 3;
        let popup_x = (f.size().width - popup_width) / 2;
        let popup_y = (f.size().height - popup_height) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        let command_popup = Paragraph::new(app.command_input.value())
            .block(
                Block::default()
                    .title("Command")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            );
        f.render_widget(command_popup, popup_area);

        f.set_cursor(
            popup_area.x + app.command_input.visual_cursor() as u16 + 1,
            popup_area.y + 1,
        );
    }

    // Render message bar
    if let Some(message) = app.get_latest_message() {
        let (message_text, style) = match message.level {
            MessageLevel::Info => (message.text.clone(), Style::default().fg(Color::Blue)),
            MessageLevel::Success => (message.text.clone(), Style::default().fg(Color::Green)),
            MessageLevel::Warning => (message.text.clone(), Style::default().fg(Color::Yellow)),
            MessageLevel::Error => (message.text.clone(), Style::default().fg(Color::Red)),
        };

        let message_bar = Paragraph::new(message_text)
            .block(
                Block::default()
                    .title("Messages")
                    .borders(Borders::ALL)
                    .border_style(style),
            );
        f.render_widget(message_bar, chunks[2]);
    } else {
        // Render empty message bar
        let empty_message_bar = Paragraph::new("")
            .block(
                Block::default()
                    .title("Messages")
                    .borders(Borders::ALL),
            );
        f.render_widget(empty_message_bar, chunks[2]);
    }

    // Render error popup if there's an error and show_error_popup is true
    if app.show_error_popup {
        if let Some(error) = &app.last_error {
            let popup_width = 60.min(f.size().width.saturating_sub(4));
            let popup_height = 3;
            let popup_x = (f.size().width - popup_width) / 2;
            let popup_y = (f.size().height - popup_height) / 2;

            let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

            let error_popup = Paragraph::new(error.as_str())
                .block(
                    Block::default()
                        .title("Error")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                );
            f.render_widget(error_popup, popup_area);
        }
    }
}
