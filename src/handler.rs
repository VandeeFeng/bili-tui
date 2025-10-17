use crate::app::{App, Focusable, InputMode, StateHandler, FocusNavigation, CommonKeyResult};
use crate::command;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use std::io;
use tui_input::backend::crossterm::EventHandler;

pub async fn handle_key_event(app: &mut App, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<bool> {
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
        InputMode::Normal => handle_normal_mode(app, key, tx)?,
        InputMode::Editing => handle_editing_mode(app, key, tx)?,
        InputMode::Detail => handle_detail_mode(app, key)?,
        InputMode::ListNav => handle_list_nav_mode(app, key)?,
    }

    Ok(false)
}

fn handle_normal_mode(app: &mut App, key: crossterm::event::KeyEvent, _tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<()> {
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
            Focusable::None => {}
        },
        KeyCode::Char('/') => {
            app.focused_panel = Focusable::Search;
            app.mode = InputMode::Editing;
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