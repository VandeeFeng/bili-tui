use crate::app::{App, Focusable, InputMode, StateHandler, FocusNavigation, CommonKeyResult};
use crate::command;
use crate::api;
use ratatui::crossterm::event::{KeyCode, KeyEventKind, Event};
use std::io;
use tui_input::backend::crossterm::EventHandler;

async fn fetch_author_dynamics(app: &mut App, uid: u64) {
    if app.loading_dynamics {
        return; // Already loading
    }

    app.loading_dynamics = true;

    match api::get_user_dynamics(uid).await {
        Ok(dynamics) => {
            let count = dynamics.len();
            app.selected_author_dynamics = Some(dynamics);
            app.dynamics_scroll_offset = 0; // Reset scroll offset when loading new dynamics
            app.add_message(format!("Successfully loaded {} dynamics", count), crate::app::MessageLevel::Success);
        }
        Err(e) => {
            app.add_message(format!("Failed to load dynamics: {}", e), crate::app::MessageLevel::Error);
            app.selected_author_dynamics = None;
            app.dynamics_scroll_offset = 0; // Reset scroll offset on error
        }
    }

    app.loading_dynamics = false;
}

pub async fn handle_key_event(app: &mut App, key: ratatui::crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<bool> {
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    // Handle command mode as overlay
    if app.command_active {
        return handle_command_mode(app, key).await;
    }

    // Handle help mode as overlay
    if app.help_active {
        return handle_help_mode(app, key);
    }

    match app.mode {
        InputMode::Normal => handle_normal_mode(app, key, tx).await?,
        InputMode::Editing => handle_editing_mode(app, key, tx)?,
        InputMode::Detail => handle_detail_mode(app, key)?,
        InputMode::ListNav => handle_list_nav_mode(app, key)?,
        InputMode::Moments => {
            handle_moments_mode(app, key).await?;
        },
    }

    Ok(false)
}

async fn handle_normal_mode(app: &mut App, key: crossterm::event::KeyEvent, _tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<()> {
    // Handle common keys first
    match app.handle_common_keys(key) {
        CommonKeyResult::Handled => return Ok(()),
        CommonKeyResult::Quit => return Err(io::Error::other("quit")),
        CommonKeyResult::Continue => {} // Continue to mode-specific handling
    }

    match key.code {
        KeyCode::Char('j') => {
            app.move_focus_next();
        }
        KeyCode::Char('k') => {
            app.move_focus_prev();
        }
        KeyCode::Enter => match app.focused_panel {
            Focusable::Search => {
                app.mode = InputMode::Editing;
            }
            Focusable::Results => {
                app.mode = InputMode::ListNav;
            }
            Focusable::MomentsAuthors | Focusable::MomentsContent => {
                // Do nothing in normal mode, moments mode handles this separately
            }
            Focusable::None => {}
        },
        KeyCode::Char('/') => {
            app.focused_panel = Focusable::Search;
            app.mode = InputMode::Editing;
        }
        KeyCode::Char('m') => {
            // Execute moments command as global shortcut
            let cmd = command::Command::ShowMoments;
            let _ = command::execute(cmd, app).await;
        }
        _ => {}
    }
    Ok(())
}

fn handle_editing_mode(app: &mut App, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<()> {
    // Handle common keys first, but with custom quit behavior for editing mode
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
            return Ok(());
        }
        KeyCode::Char('?') => {
            app.activate_help();
            return Ok(());
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.focused_panel = Focusable::None;
            return Ok(());
        }
        _ => {}
    }

    match key.code {
        KeyCode::Enter => {
            let query = app.search_input.value().to_string();
            let tx = tx.clone();
            tokio::spawn(async move {
                let response = match crate::api::search(&query).await {
                    Ok(results) => Ok(results),
                    Err(e) => Err(e.to_string()),
                };
                let _ = tx.send(response).await;
            });
            app.mode = InputMode::Normal;
        }
        _ => {
            app.search_input.handle_event(&Event::Key(key));
        }
    }
    Ok(())
}

async fn handle_command_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Enter => {
            let command_str = app.command_input.value().to_string();
            app.command_input.reset();
            app.command_active = false;

            match command::parse(&command_str) {
                Ok(cmd) => {
                    if let command::Command::Quit = cmd {
                        return Ok(true);
                    }
                    match command::execute(cmd, app).await {
                        Ok(_) => {
                            app.add_message("Command executed successfully".to_string(), crate::app::MessageLevel::Success);
                        }
                        Err(e) => {
                            app.add_message(format!("Command execution failed: {}", e), crate::app::MessageLevel::Error);
                        }
                    }
                }
                Err(e) => {
                    app.add_message(format!("Command parsing error: {}", e), crate::app::MessageLevel::Error);
                }
            }
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.command_active = false;
            // Don't change the current mode or focused_panel, just deactivate command
        }
        _ => {
            app.command_input.handle_event(&Event::Key(key));
        }
    }
    Ok(false)
}

fn handle_detail_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<()> {
    // Handle common keys first, but with custom quit behavior for detail mode
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
            return Ok(());
        }
        KeyCode::Char('?') => {
            app.activate_help();
            return Ok(());
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.focused_panel = Focusable::None;
            app.video_info = None;
            return Ok(());
        }
        _ => {}
    }

    match key.code {
        KeyCode::Char('j') => {
            app.move_focus_next();
        }
        KeyCode::Char('k') => {
            app.move_focus_prev();
        }
        KeyCode::Enter => if app.focused_panel == Focusable::Search {
            app.mode = InputMode::Editing;
        },
        KeyCode::Char('p') => {
            app.play_video();
        }
        _ => {}
    }
    Ok(())
}

fn handle_list_nav_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<()> {
    // Handle common keys first, but with custom quit behavior for list nav mode
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
            return Ok(());
        }
        KeyCode::Char('?') => {
            app.activate_help();
            return Ok(());
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.results_list_state.select(None);
            app.focused_panel = Focusable::None;
            return Ok(());
        }
        _ => {}
    }

    match key.code {
        KeyCode::Char('j') => {
            if !app.search_results.is_empty() {
                let i = match app.results_list_state.selected() {
                    Some(i) => {
                        if i >= app.search_results.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                app.results_list_state.select(Some(i));
            }
        }
        KeyCode::Char('k') => {
            if !app.search_results.is_empty() {
                let i = match app.results_list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            app.search_results.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                app.results_list_state.select(Some(i));
            }
        }
        KeyCode::Enter => {
            app.mode = InputMode::Detail;
        }
        _ => {}
    }
    Ok(())
}

fn handle_help_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.help_active = false;
            // Don't change the current mode, just deactivate help
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_moments_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<()> {
    // Handle common keys first, but with custom quit behavior for moments mode
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
            return Ok(());
        }
        KeyCode::Char('?') => {
            app.activate_help();
            return Ok(());
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.moments_active = false;
            app.focused_panel = Focusable::None;
            app.selected_author.select(None);
            return Ok(());
        }
        _ => {}
    }

    match key.code {
        KeyCode::Char('j') => {
            if app.focused_panel == Focusable::MomentsAuthors {
                // Navigate down in authors list
                if let Some(data) = &app.moments_data {
                    if !data.is_empty() {
                        let i = match app.selected_author.selected() {
                            Some(i) => {
                                if i >= data.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        let _previous_index = app.selected_author.selected();
                        app.selected_author.select(Some(i));

                        // Fetch dynamics for the newly selected author
                        if let Some(data) = &app.moments_data {
                            if let Some(author) = data.get(i) {
                                let uid = author.user_profile.info.uid;
                                fetch_author_dynamics(app, uid).await;
                            }
                        }
                    }
                }
            } else if app.focused_panel == Focusable::MomentsContent {
                // Scroll down in dynamics content
                if let Some(dynamics) = &app.selected_author_dynamics {
                    if app.dynamics_scroll_offset + 1 < dynamics.len() {
                        app.dynamics_scroll_offset += 1;
                    }
                }
            } else {
                // Move focus to next panel
                app.move_focus_next();
            }
        }
        KeyCode::Char('k') => {
            if app.focused_panel == Focusable::MomentsAuthors {
                // Navigate up in authors list
                if let Some(data) = &app.moments_data {
                    if !data.is_empty() {
                        let i = match app.selected_author.selected() {
                            Some(i) => {
                                if i == 0 {
                                    data.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        let _previous_index = app.selected_author.selected();
                        app.selected_author.select(Some(i));

                        // Fetch dynamics for the newly selected author
                        if let Some(data) = &app.moments_data {
                            if let Some(author) = data.get(i) {
                                let uid = author.user_profile.info.uid;
                                fetch_author_dynamics(app, uid).await;
                            }
                        }
                    }
                }
            } else if app.focused_panel == Focusable::MomentsContent {
                // Scroll up in dynamics content
                if app.dynamics_scroll_offset > 0 {
                    app.dynamics_scroll_offset -= 1;
                }
            } else {
                // Move focus to previous panel
                app.move_focus_prev();
            }
        }
        KeyCode::Tab => {
            // Legacy Tab key - switch between author and content panels
            match app.focused_panel {
                Focusable::MomentsAuthors => app.focused_panel = Focusable::MomentsContent,
                Focusable::MomentsContent => app.focused_panel = Focusable::MomentsAuthors,
                _ => app.focused_panel = Focusable::MomentsAuthors,
            }
        }
        KeyCode::Char('h') => {
            if app.focused_panel == Focusable::MomentsContent {
                app.focused_panel = Focusable::MomentsAuthors;
            }
        }
        KeyCode::Char('l') => {
            if app.focused_panel == Focusable::MomentsAuthors {
                app.focused_panel = Focusable::MomentsContent;
            }
        }
        KeyCode::Up | KeyCode::Down => {
            // Arrow keys also work for navigation in authors list
            if app.focused_panel == Focusable::MomentsAuthors {
                if let Some(data) = &app.moments_data && !data.is_empty() {
                    let current = app.selected_author.selected().unwrap_or(0);
                    let new_index = match key.code {
                        KeyCode::Up => {
                            if current == 0 { data.len() - 1 } else { current - 1 }
                        }
                        KeyCode::Down => {
                            if current >= data.len() - 1 { 0 } else { current + 1 }
                        }
                        _ => current,
                    };
                    let previous_index = app.selected_author.selected();
                    app.selected_author.select(Some(new_index));

                    // Fetch dynamics for the newly selected author if different
                    if previous_index != Some(new_index) {
                        if let Some(data) = &app.moments_data {
                            if let Some(author) = data.get(new_index) {
                                let uid = author.user_profile.info.uid;
                                fetch_author_dynamics(app, uid).await;
                            }
                        }
                    }
                }
            }
        }
        KeyCode::Left | KeyCode::Right => {
            // Arrow keys for panel switching
            match key.code {
                KeyCode::Left => app.focused_panel = Focusable::MomentsAuthors,
                KeyCode::Right => app.focused_panel = Focusable::MomentsContent,
                _ => {}
            }
        }
        KeyCode::Enter => {
            // Load dynamics for selected author
            if let (Some(data), Some(selected_index)) = (&app.moments_data, app.selected_author.selected()) {
                if let Some(author) = data.get(selected_index) {
                    let uid = author.user_profile.info.uid;
                    fetch_author_dynamics(app, uid).await;
                }
            }
        }
        _ => {}
    }
    Ok(())
}