use crate::app::{App, Focusable};
use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem};

pub fn render_results_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let results: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(index, video)| {
            let text_width = area.width.saturating_sub(6) as usize;
            let options = textwrap::Options::new(text_width)
                .initial_indent("")
                .subsequent_indent("  ");

            let title_wrapped = textwrap::wrap(&video.title, options);

            // Check if this item is selected
            let is_selected = app.results_list_state.selected() == Some(index);

            let mut lines: Vec<Line> = title_wrapped
                .iter()
                .map(|s| {
                    if is_selected {
                        Line::from(s.to_string().fg(Color::Green))
                    } else {
                        Line::from(s.to_string())
                    }
                })
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

    let results_focused = app.focused_panel() == Focusable::Results;
    let is_list_nav = app.input_mode() == crate::app::InputMode::ListNav;

    // Calculate block color based on focus and mode
    let block_color = if results_focused && is_list_nav {
        Color::Cyan
    } else if results_focused {
        Color::Green
    } else {
        Color::Reset
    };

    let results_list = List::new(results)
        .block(
            ratatui::widgets::Block::default()
                .title("Results")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(block_color))
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(results_list, area, &mut app.results_list_state);
}
