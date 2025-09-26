use crate::app::{App, Focusable, InputMode};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
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
                .block(Block::default().title("Video Details").borders(Borders::ALL));
            f.render_widget(info_panel, chunks[1]);
        }
        _ => {
            let results: Vec<ListItem> = app
                .search_results
                .iter()
                .map(|video| {
                    let content = format!(
                        "{} - {} (▶ {})",
                        video.title,
                        video.author,
                        video.play.to_string().trim_matches('"')
                    );
                    ListItem::new(content)
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

    if let Some(error) = &app.last_error {
        let command_line = Paragraph::new(error.as_str()).block(
            Block::default()
                .title("Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        );
        f.render_widget(command_line, chunks[2]);
    } else {
        let command_line = Paragraph::new(app.command_input.value()).block(
            Block::default()
                .title("Command")
                .borders(Borders::ALL)
                .border_style(if app.focused_panel == Focusable::Command {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                }),
        );
        f.render_widget(command_line, chunks[2]);
    }

    if app.is_commanding() {
        f.set_cursor(
            chunks[2].x + app.command_input.visual_cursor() as u16 + 1,
            chunks[2].y + 1,
        );
    }
}
