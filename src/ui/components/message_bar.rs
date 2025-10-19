use crate::app::App;
use crate::ui::traits::MessageRenderer;
use ratatui::prelude::*;

pub fn render_message_bar(f: &mut Frame, area: Rect, app: &App) {
    app.render_message_bar(f, area, app);
}