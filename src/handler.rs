use crate::app::{App, Focusable, InputMode, StateHandler, FocusNavigation, CommonKeyResult};
use crate::command;
use crate::api;
use crossterm::event::{KeyCode, KeyEventKind, KeyEvent as CrosstermKeyEvent};
use ratatui::crossterm::event::{Event, KeyEvent as RatatuiKeyEvent, KeyEventKind as RatatuiKeyEventKind};
use std::io;
use tui_input::backend::crossterm::EventHandler;

// Convert crossterm event to ratatui's crossterm event
fn convert_key_event(key: CrosstermKeyEvent) -> RatatuiKeyEvent {
    use std::mem::transmute;

    RatatuiKeyEvent {
        code: unsafe { transmute::<crossterm::event::KeyCode, ratatui::crossterm::event::KeyCode>(key.code) },
        modifiers: unsafe { transmute::<crossterm::event::KeyModifiers, ratatui::crossterm::event::KeyModifiers>(key.modifiers) },
        kind: match key.kind {
            KeyEventKind::Press => RatatuiKeyEventKind::Press,
            KeyEventKind::Repeat => RatatuiKeyEventKind::Repeat,
            KeyEventKind::Release => RatatuiKeyEventKind::Release,
        },
        state: unsafe { transmute::<crossterm::event::KeyEventState, ratatui::crossterm::event::KeyEventState>(key.state) },
    }
}

// Common navigation pattern for list selection
fn navigate_list(index: Option<usize>, len: usize, direction: bool) -> usize {
    if len == 0 {
        return 0;
    }

    let current = index.unwrap_or(0);
    if direction { // Down/Next
        if current >= len - 1 { 0 } else { current + 1 }
    } else { // Up/Prev
        if current == 0 { len - 1 } else { current - 1 }
    }
}

// Handle common overlay keys (command, help, quit)
fn handle_overlay_keys(app: &mut App, key: crossterm::event::KeyEvent) -> Option<bool> {
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
            Some(false)
        }
        KeyCode::Char('?') => {
            app.activate_help();
            Some(false)
        }
        KeyCode::Char('q') | KeyCode::Esc => Some(true),
        _ => None,
    }
}

// Handle global shortcuts that work across all pages
fn handle_global_shortcuts(app: &mut App, key: crossterm::event::KeyEvent, _tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> Option<bool> {
    match key.code {
        // Global search shortcut - works on any page
        KeyCode::Char('/') => {
            app.focused_panel = Focusable::Search;
            app.mode = InputMode::Editing;
            Some(false)
        }
        // Other global shortcuts handled by page-specific handlers
        // This includes 'm' for moments which is handled in each page handler
        _ => None,
    }
}

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

pub async fn handle_key_event(app: &mut App, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<bool> {
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    // Handle command mode as overlay
    if app.command_active {
        return handle_command_mode(app, key).await;
    }

    // Handle global shortcuts first - they work even when help is active
    if let Some(result) = handle_global_shortcuts(app, key, tx) {
        return Ok(result);
    }

    // Handle help mode as overlay (but after global shortcuts)
    if app.help_active {
        return handle_help_mode(app, key);
    }

    // Handle editing mode (for search input)
    if app.mode == InputMode::Editing {
        return handle_editing_mode(app, key, tx).map(|_| false);
    }

    // Handle list navigation mode (for results list)
    if app.mode == InputMode::ListNav {
        return handle_list_nav_mode(app, key).map(|_| false);
    }

    // Delegate to current page
    match app.active_page {
        crate::app::ActivePage::Search => handle_search_page(app, key, tx).await?,
        crate::app::ActivePage::Moments => handle_moments_page(app, key).await?,
        crate::app::ActivePage::Detail => handle_detail_page(app, key).await?,
    }

    Ok(false)
}

// Search page handler
async fn handle_search_page(app: &mut App, key: crossterm::event::KeyEvent, _tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<()> {
    // Handle common keys first
    match app.handle_common_keys(key) {
        CommonKeyResult::Handled => return Ok(()),
        CommonKeyResult::Quit => return Err(io::Error::other("quit")),
        CommonKeyResult::Continue => {} // Continue to page-specific handling
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
            _ => {}
        },
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
    if let Some(should_quit) = handle_overlay_keys(app, key) {
        if should_quit {
            app.mode = InputMode::Normal;
            app.focused_panel = Focusable::Search; // Return focus to search
        }
        return Ok(());
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
            // Switch to search page to show results
            app.active_page = crate::app::ActivePage::Search;
        }
        _ => {
            let ratatui_key = convert_key_event(key);
            app.search_input.handle_event(&Event::Key(ratatui_key));
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
            let ratatui_key = convert_key_event(key);
            app.command_input.handle_event(&Event::Key(ratatui_key));
        }
    }
    Ok(false)
}

// Detail page handler
async fn handle_detail_page(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<()> {
    // Handle common keys first
    match app.handle_common_keys(key) {
        CommonKeyResult::Handled => return Ok(()),
        CommonKeyResult::Quit => {
            // Return to search page when quitting detail
            app.active_page = crate::app::ActivePage::Search;
            app.focused_panel = Focusable::Results;
            app.video_info = None;
            return Ok(());
        },
        CommonKeyResult::Continue => {} // Continue to page-specific handling
    }

    match key.code {
        KeyCode::Char('j') => app.move_focus_next(),
        KeyCode::Char('k') => app.move_focus_prev(),
        KeyCode::Enter => if app.focused_panel == Focusable::Search {
            app.mode = InputMode::Editing;
        },
        KeyCode::Char('p') => app.play_video(),
        KeyCode::Char('m') => {
            // Execute moments command
            let cmd = command::Command::ShowMoments;
            let _ = command::execute(cmd, app).await;
        }
        _ => {}
    }
    Ok(())
}


fn handle_list_nav_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<()> {
    if let Some(should_quit) = handle_overlay_keys(app, key) {
        if should_quit {
            app.mode = InputMode::Normal;
            app.results_list_state.select(None);
            app.focused_panel = Focusable::None;
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let i = navigate_list(app.results_list_state.selected(), app.search_results.len(), true);
            app.results_list_state.select(Some(i));
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = navigate_list(app.results_list_state.selected(), app.search_results.len(), false);
            app.results_list_state.select(Some(i));
        }
        KeyCode::Enter => {
            // Set video info and switch to detail page
            if let Some(selected_index) = app.results_list_state.selected() {
                if let Some(video) = app.search_results.get(selected_index) {
                    // Store basic video info, will be fully loaded in detail page
                    app.video_info = Some(crate::api::VideoInfo {
                        bvid: video.bvid.clone(),
                        title: video.title.clone(),
                        desc: video.description.clone(),
                        owner: crate::api::Owner {
                            name: video.author.clone(),
                        },
                        stat: crate::api::Stat {
                            view: 0, // Will be populated when fully loaded
                            like: video.like,
                            coin: 0,
                            favorite: 0,
                            share: 0,
                        },
                    });
                }
            }
            app.active_page = crate::app::ActivePage::Detail;
            app.mode = InputMode::Normal;
        }
        // Handle global shortcuts
        KeyCode::Char('/') => {
            app.mode = InputMode::Editing;
            app.focused_panel = Focusable::Search;
        }
        _ => {}
    }
    Ok(())
}

fn handle_help_mode(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Char(':') => {
            app.activate_command();
            Ok(false)
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.help_active = false;
            Ok(false)
        }
        // Allow search shortcut even in help mode
        KeyCode::Char('/') => {
            app.help_active = false; // Close help first
            app.focused_panel = Focusable::Search;
            app.mode = InputMode::Editing;
            Ok(false)
        }
        // Allow moments shortcut even in help mode
        KeyCode::Char('m') => {
            app.help_active = false; // Close help first
            // Note: moments command will be handled by the page handlers after help closes
            Ok(false)
        }
        _ => Ok(false),
    }
}

// Moments page handler
async fn handle_moments_page(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<()> {
    // Handle common keys first
    match app.handle_common_keys(key) {
        CommonKeyResult::Handled => return Ok(()),
        CommonKeyResult::Quit => {
            // Return to search page when quitting moments
            app.active_page = crate::app::ActivePage::Search;
            app.focused_panel = Focusable::Search;
            app.selected_author.select(None);
            return Ok(());
        },
        CommonKeyResult::Continue => {} // Continue to page-specific handling
    }

    match key.code {
        KeyCode::Char('j') => {
            if app.focused_panel == Focusable::MomentsAuthors {
                // Navigate down in authors list
                if let Some(data) = &app.moments_data && !data.is_empty() {
                    let i = navigate_list(app.selected_author.selected(), data.len(), true);
                    app.selected_author.select(Some(i));

                    if let Some(author) = data.get(i) {
                        fetch_author_dynamics(app, author.user_profile.info.uid).await;
                    }
                }
            } else if app.focused_panel == Focusable::MomentsContent {
                // Scroll down in dynamics content
                if let Some(dynamics) = &app.selected_author_dynamics
                    && app.dynamics_scroll_offset + 1 < dynamics.len() {
                        app.dynamics_scroll_offset += 1;
                    }
            } else {
                app.move_focus_next();
            }
        }
        KeyCode::Char('k') => {
            if app.focused_panel == Focusable::MomentsAuthors {
                // Navigate up in authors list
                if let Some(data) = &app.moments_data && !data.is_empty() {
                    let i = navigate_list(app.selected_author.selected(), data.len(), false);
                    app.selected_author.select(Some(i));

                    if let Some(author) = data.get(i) {
                        fetch_author_dynamics(app, author.user_profile.info.uid).await;
                    }
                }
            } else if app.focused_panel == Focusable::MomentsContent {
                // Scroll up in dynamics content
                if app.dynamics_scroll_offset > 0 {
                    app.dynamics_scroll_offset -= 1;
                }
            } else {
                app.move_focus_prev();
            }
        }
        KeyCode::Tab => {
            // Switch between author and content panels
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
            if app.focused_panel == Focusable::MomentsAuthors
                && let Some(data) = &app.moments_data && !data.is_empty() {
                    let direction = matches!(key.code, KeyCode::Down);
                    let new_index = navigate_list(app.selected_author.selected(), data.len(), direction);
                    let previous_index = app.selected_author.selected();
                    app.selected_author.select(Some(new_index));

                    if previous_index != Some(new_index)
                        && let Some(author) = data.get(new_index) {
                            fetch_author_dynamics(app, author.user_profile.info.uid).await;
                        }
                } else if app.focused_panel == Focusable::MomentsContent {
                // Handle up/down arrows for scrolling in content panel
                if let Some(dynamics) = &app.selected_author_dynamics {
                    if key.code == KeyCode::Down && app.dynamics_scroll_offset + 1 < dynamics.len() {
                        app.dynamics_scroll_offset += 1;
                    } else if key.code == KeyCode::Up && app.dynamics_scroll_offset > 0 {
                        app.dynamics_scroll_offset -= 1;
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
            if let (Some(data), Some(selected_index)) = (&app.moments_data, app.selected_author.selected())
                && let Some(author) = data.get(selected_index) {
                    let uid = author.user_profile.info.uid;
                    fetch_author_dynamics(app, uid).await;
                }
        }
        _ => {}
    }
    Ok(())
}

