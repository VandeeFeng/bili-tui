use crate::app::App;
use crate::ui::traits::WidgetRenderer;
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph};

pub fn render_help_popup(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from("Commands:".bold()),
        Line::from("  video <url>        - Play video with mpv"),
        Line::from("  video-info <url>   - Show video details"),
        Line::from("  moments (or m)     - Show following authors' updates"),
        Line::from("  help               - Show this help message"),
        Line::from("  q                  - Quit the application"),
        Line::from(""),
        Line::from("Navigation:".bold()),
        Line::from("  j/k                - Move focus between panels"),
        Line::from("  /                  - Search"),
        Line::from("  m                  - Moments (following authors)"),
        Line::from("  Enter              - Select/Enter panel"),
        Line::from("  q/Esc              - Exit current mode/panel"),
        Line::from("  :                  - Open command popup"),
        Line::from("  ?                  - Show help"),
        Line::from(""),
        Line::from("Moments Mode:".bold()),
        Line::from("  j/k                - Navigate authors list or scroll dynamics content"),
        Line::from("  h/l                - Switch between author/content panels"),
        Line::from("  Tab                - Legacy: switch between panels"),
        Line::from("  q/Esc              - Exit moments mode"),
    ];
    let help_panel = Paragraph::new(help_text)
        .block(ratatui::widgets::Block::default().title("Help").borders(ratatui::widgets::Borders::ALL));
    f.render_widget(help_panel, area);
}

pub fn render_command_popup(f: &mut Frame, app: &mut App) {
    let popup_area = app.calculate_popup_area(f.area(), 60, 3);

    // Clear the background area to create a clean background
    f.render_widget(Clear, popup_area);

    let command_popup = Paragraph::new(app.command_input.value())
        .block(app.create_popup_block("Command", Color::Green));
    f.render_widget(command_popup, popup_area);

    f.set_cursor_position(
        (
            popup_area.x + app.command_input.visual_cursor() as u16 + 1,
            popup_area.y + 1,
        )
    );
}

pub fn render_error_popup(f: &mut Frame, app: &App) {
    if let Some(error) = &app.last_error {
        let popup_area = app.calculate_popup_area(f.area(), 60, 3);

        let error_popup = Paragraph::new(error.as_str())
            .block(app.create_popup_block("Error", Color::Red));
        f.render_widget(error_popup, popup_area);
    }
}