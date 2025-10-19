use crate::app::{App, Focusable};
use crate::ui::traits::WidgetRenderer;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_video_details_panel(f: &mut Frame, area: Rect, app: &App) {
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
    f.render_widget(info_panel, area);
}