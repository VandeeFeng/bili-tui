use crate::app::{App, Focusable};
use crate::ui::traits::WidgetRenderer;
use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem};

pub fn render_results_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let results: Vec<ListItem> = app
        .search_results
        .iter()
        .map(|video| {
            let text_width = area.width.saturating_sub(6) as usize;
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

    f.render_stateful_widget(results_list, area, &mut app.results_list_state);
}