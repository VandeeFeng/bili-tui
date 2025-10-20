use crate::app::App;
use crate::ui::traits::WidgetRenderer;
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph};

pub fn render_help_popup(f: &mut Frame, app: &App) {
    // Calculate popup area to occupy 70% of terminal
    let popup_width = f.area().width * 70 / 100;
    let popup_height = f.area().height * 70 / 100;
    let popup_area = app.calculate_popup_area(f.area(), popup_width, popup_height);

    // Clear the background area to create a clean overlay
    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from("Bili-TUI - Bilibili Terminal UI Help".bold().cyan()),
        Line::from(""),

        Line::from("Global Shortcuts:".bold().yellow()),
        Line::from("  /                  - Search (global shortcut)"),
        Line::from("  m                  - Moments/Following updates"),
        Line::from("  M                  - Show messages popup"),
        Line::from("  :                  - Command mode"),
        Line::from("  ?                  - Show this help"),
        Line::from("  q/Esc              - Exit current mode or quit"),
        Line::from(""),

        Line::from("Navigation:".bold().yellow()),
        Line::from("  j/k                - Move focus between panels"),
        Line::from("  h/l                - Switch panels (in Moments mode)"),
        Line::from("  ←/→               - Switch panels (alternative)"),
        Line::from("  Tab                - Legacy panel switching"),
        Line::from("  Enter              - Activate/Select current panel"),
        Line::from(""),

        Line::from("Commands:".bold().yellow()),
        Line::from("  video <url>        - Play video with mpv"),
        Line::from("  video-info <url>   - Show video details"),
        Line::from("  moments (or m)     - Show following authors' updates"),
        Line::from("  add <uid> <name>   - Add author to custom following"),
        Line::from("  remove <uid>       - Remove author from custom following"),
        Line::from("  ban <uid> <name>   - Add author to blacklist"),
        Line::from("  unban <uid>        - Remove author from blacklist"),
        Line::from("  list               - Show following and blacklist status"),
        Line::from("  refresh            - Refresh authors from API (SESSDATA only)"),
        Line::from("  toggle-custom      - Toggle custom following mode"),
        Line::from("  help               - Show this help message"),
        Line::from("  q                  - Quit the application"),
        Line::from(""),

        Line::from("Search Mode:".bold().yellow()),
        Line::from("  /                  - Start searching"),
        Line::from("  Enter (editing)    - Execute search"),
        Line::from("  ↓/j/k/↑           - Navigate results"),
        Line::from("  Enter (results)    - View video details"),
        Line::from(""),

        Line::from("Moments Mode:".bold().yellow()),
        Line::from("  j/k / ↑/↓          - Navigate authors or dynamics list (selection)"),
        Line::from("  h/l                - Switch between author/content panels"),
        Line::from("  Shift+J/K / Shift+↑/↓ - Scroll dynamics content view"),
        Line::from("  Enter (authors)    - Load author's dynamics"),
        Line::from("  p                  - Play selected dynamic video"),
        Line::from("  q/Esc              - Exit moments mode"),
        Line::from(""),

        Line::from("Detail View:".bold().yellow()),
        Line::from("  p                  - Play current video"),
        Line::from("  j/k                - Move focus between panels"),
        Line::from("  q/Esc              - Return to search"),
        Line::from(""),

        Line::from("Press q/Esc to close help".italic().gray()),
    ];

    let help_panel = Paragraph::new(help_text)
        .block(app.create_popup_block("Help", Color::Cyan))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(help_panel, popup_area);
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

pub fn render_messages_popup(f: &mut Frame, app: &App) {
    // Calculate popup area to occupy 70% of terminal (same as help)
    let popup_width = f.area().width * 70 / 100;
    let popup_height = f.area().height * 70 / 100;
    let popup_area = app.calculate_popup_area(f.area(), popup_width, popup_height);

    // Clear the background area to create a clean overlay
    f.render_widget(Clear, popup_area);

    if app.messages.is_empty() {
        let empty_text = vec![
            Line::from("Messages".bold().cyan()),
            Line::from(""),
            Line::from("No messages yet.".italic().gray()),
        ];

        let messages_panel = Paragraph::new(empty_text)
            .block(app.create_popup_block("Messages", Color::Cyan))
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(messages_panel, popup_area);
        return;
    }

    let mut message_lines = vec![
        Line::from(format!("Messages ({})", app.messages.len()).bold().cyan()),
        Line::from(""),
    ];

    for (index, message) in app.messages.iter().enumerate() {
        let message_text = match message.level {
            crate::app::MessageLevel::Info => Line::from(message.text.clone()).cyan(),
            crate::app::MessageLevel::Success => Line::from(message.text.clone()).green(),
            crate::app::MessageLevel::Warning => Line::from(message.text.clone()).yellow(),
            crate::app::MessageLevel::Error => Line::from(message.text.clone()).red(),
        };

        message_lines.push(message_text);

        // Add spacing between messages (but not after the last one)
        if index < app.messages.len() - 1 {
            message_lines.push(Line::from(""));
        }
    }

    let messages_panel = Paragraph::new(message_lines)
        .block(app.create_popup_block("Messages", Color::Cyan))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(messages_panel, popup_area);
}

pub fn render_error_popup(f: &mut Frame, app: &App) {
    if let Some(error) = &app.last_error {
        let popup_area = app.calculate_popup_area(f.area(), 60, 3);

        let error_popup = Paragraph::new(error.as_str())
            .block(app.create_popup_block("Error", Color::Red));
        f.render_widget(error_popup, popup_area);
    }
}