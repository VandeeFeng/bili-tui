use crate::app::App;
use ratatui::prelude::*;

pub mod traits;
pub mod components;

pub use components::*;

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),     // Search bar
            Constraint::Min(0),        // Main content area
            Constraint::Length(3),     // Message bar
        ])
        .split(f.area());

    render_search_panel(f, chunks[0], app);

    match app.active_page() {
        crate::app::ActivePage::Detail => {
            render_video_details_panel(f, chunks[1], app);
        }
        crate::app::ActivePage::Moments => {
            render_moments_panel(f, chunks[1], app);
        }
        crate::app::ActivePage::Search => {
            render_results_panel(f, chunks[1], app);
        }
    }

    // Render help popup if active
    if app.overlays.help {
        render_help_popup(f, app);
    }

    // Render command popup if active
    if app.is_commanding() {
        render_command_popup(f, app);
    }

    // Render message bar
    render_message_bar(f, chunks[2], app);

    // Render error popup if there's an error and show_error_popup is true
    if app.show_error_popup {
        render_error_popup(f, app);
    }
}