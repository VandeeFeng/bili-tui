use crate::app::{App, Focusable};
use crate::ui::traits::WidgetRenderer;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_search_panel(f: &mut Frame, area: Rect, app: &App) {
    let search_focused = app.focused_panel() == Focusable::Search;
    let search_bar = Paragraph::new(app.search_input.value())
        .block(app.create_focused_block("Search", search_focused));
    f.render_widget(search_bar, area);

    if app.is_editing() {
        f.set_cursor_position((
            area.x + app.search_input.visual_cursor() as u16 + 1,
            area.y + 1,
        ));
    }
}
