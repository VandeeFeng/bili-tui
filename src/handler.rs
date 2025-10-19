use crate::app::{App, NavigationHandler, NavigationAction, NavigationResult, ActivePage, Focusable, InputMode, MessageLevel};
use ratatui::crossterm::event::{KeyEvent as RatatuiKeyEvent};
use std::io;

// Convert crossterm event to ratatui's crossterm event for input handling
fn convert_key_event_for_input(key: crossterm::event::KeyEvent) -> RatatuiKeyEvent {
    use std::mem::transmute;

    RatatuiKeyEvent {
        code: unsafe { transmute::<crossterm::event::KeyCode, ratatui::crossterm::event::KeyCode>(key.code) },
        modifiers: unsafe { transmute::<crossterm::event::KeyModifiers, ratatui::crossterm::event::KeyModifiers>(key.modifiers) },
        kind: unsafe { transmute::<crossterm::event::KeyEventKind, ratatui::crossterm::event::KeyEventKind>(key.kind) },
        state: unsafe { transmute::<crossterm::event::KeyEventState, ratatui::crossterm::event::KeyEventState>(key.state) },
    }
}

/// Main keyboard event handler - delegates to App's NavigationHandler implementation
pub async fn handle_key_event(app: &mut App, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> io::Result<bool> {
    app.handle_key(key, tx).await
}

impl NavigationHandler for App {
    async fn handle_key(&mut self, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> std::io::Result<bool> {
        use crossterm::event::{KeyCode, KeyEventKind};

        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }

        // Handle overlay modes first
        if self.overlays.command {
            return self.handle_command_mode(key).await;
        }

        // Handle help overlay
        if self.overlays.help {
            return self.handle_help_mode(key).await;
        }

        // Handle editing mode
        if self.navigation.input_mode == InputMode::Editing {
            return self.handle_editing_mode(key, tx).await.map(|_| false);
        }

        // Convert key to navigation action
        let action = match key.code {
            KeyCode::Char(':') => NavigationAction::ToggleCommand,
            KeyCode::Char('?') => NavigationAction::ToggleHelp,
            KeyCode::Char('q') | KeyCode::Esc => NavigationAction::Exit,
            KeyCode::Char('j') if self.can_navigate_panels() => NavigationAction::PanelNext,
            KeyCode::Char('k') if self.can_navigate_panels() => NavigationAction::PanelPrev,
            KeyCode::Char('j') | KeyCode::Down if self.can_navigate_list() => NavigationAction::ListDown,
            KeyCode::Char('k') | KeyCode::Up if self.can_navigate_list() => NavigationAction::ListUp,
            KeyCode::Enter => NavigationAction::Activate,
            KeyCode::Char('/') => {
                self.set_focused_panel(Focusable::Search);
                self.set_input_mode(InputMode::Editing);
                return Ok(false);
            }
            KeyCode::Char('m') => {
                // Handle moments command
                let cmd = crate::command::Command::ShowMoments;
                let _ = crate::command::execute(cmd, self).await;
                return Ok(false);
            }
            KeyCode::Char('p') => {
                self.play_video();
                return Ok(false);
            }
            // Horizontal navigation for moments panels
            KeyCode::Char('h') | KeyCode::Left if self.navigation.current_page == ActivePage::Moments => NavigationAction::PanelLeft,
            KeyCode::Char('l') | KeyCode::Right if self.navigation.current_page == ActivePage::Moments => NavigationAction::PanelRight,
            _ => return Ok(false),
        };

        // Execute navigation action
        match self.execute_navigation(action) {
            NavigationResult::Quit => Ok(true),
            NavigationResult::Handled | NavigationResult::Continue => Ok(false),
        }
    }

    fn execute_navigation(&mut self, action: NavigationAction) -> NavigationResult {
        match action {
            NavigationAction::ToggleCommand => {
                self.overlays.command = true;
                self.command_input.reset();
                NavigationResult::Handled
            }
            NavigationAction::ToggleHelp => {
                self.overlays.help = true;
                NavigationResult::Handled
            }
            NavigationAction::Exit => {
                if self.navigation.input_mode == InputMode::ListNav {
                    self.exit_list_nav_mode();
                    NavigationResult::Handled
                } else if self.navigation.current_page == ActivePage::Detail {
                    // Return to search page when quitting detail
                    self.set_active_page(ActivePage::Search);
                    self.set_focused_panel(Focusable::Results);
                    self.video_info = None;
                    NavigationResult::Handled
                } else if self.navigation.current_page == ActivePage::Moments {
                    // Return to search page when quitting moments
                    self.set_active_page(ActivePage::Search);
                    self.set_focused_panel(Focusable::Search);
                    self.selected_author.select(None);
                    NavigationResult::Handled
                } else {
                    NavigationResult::Quit
                }
            }
            NavigationAction::PanelNext => {
                self.navigation.focused_panel = self.navigation.focused_panel.next();
                NavigationResult::Handled
            }
            NavigationAction::PanelPrev => {
                self.navigation.focused_panel = self.navigation.focused_panel.prev();
                NavigationResult::Handled
            }
            NavigationAction::ListDown => self.handle_list_navigation(true),
            NavigationAction::ListUp => self.handle_list_navigation(false),
            NavigationAction::Activate => self.handle_activate(),
            NavigationAction::PanelLeft => self.handle_horizontal_navigation(false),
            NavigationAction::PanelRight => self.handle_horizontal_navigation(true),
        }
    }

    fn can_navigate_panels(&self) -> bool {
        self.navigation.input_mode != InputMode::ListNav
    }

    fn can_navigate_list(&self) -> bool {
        self.navigation.input_mode == InputMode::ListNav
    }
}

impl App {
    // Helper methods for navigation
    fn handle_list_navigation(&mut self, is_down: bool) -> NavigationResult {
        match self.navigation.current_page {
            ActivePage::Search => {
                let current = self.results_list_state.selected().unwrap_or(0);
                let new_index = if is_down {
                    if current >= self.search_results.len().saturating_sub(1) { 0 } else { current + 1 }
                } else if current == 0 { self.search_results.len().saturating_sub(1) } else { current - 1 };
                self.results_list_state.select(Some(new_index));
                NavigationResult::Handled
            }
            ActivePage::Moments => {
                if self.navigation.focused_panel == Focusable::MomentsAuthors {
                    if let Some(data) = &self.moments_data && !data.is_empty() {
                        let current = self.selected_author.selected().unwrap_or(0);
                        let new_index = if is_down {
                            if current >= data.len().saturating_sub(1) { 0 } else { current + 1 }
                        } else if current == 0 { data.len().saturating_sub(1) } else { current - 1 };
                        self.selected_author.select(Some(new_index));

                        // Load dynamics for selected author
                        if let Some(author) = data.get(new_index) {
                            let uid = author.user_profile.info.uid;
                            self.add_message(format!("Loading dynamics for UID: {}", uid), MessageLevel::Info);
                            self.loading_dynamics = true;
                            self.selected_author_dynamics = None;
                            self.dynamics_scroll_offset = 0;

                            // Simple synchronous loading for now - this blocks but works
                            match std::thread::spawn(move || {
                                tokio::runtime::Runtime::new()
                                    .unwrap()
                                    .block_on(crate::api::get_user_dynamics(uid))
                            }).join().unwrap() {
                                Ok(dynamics) => {
                                    let count = dynamics.len();
                                    self.selected_author_dynamics = Some(dynamics);
                                    self.add_message(format!("Loaded {} dynamics", count), MessageLevel::Success);
                                }
                                Err(e) => {
                                    self.add_message(format!("Failed to load dynamics: {}", e), MessageLevel::Error);
                                    self.selected_author_dynamics = None;
                                }
                            }

                            self.loading_dynamics = false;
                        }
                    }
                } else if self.navigation.focused_panel == Focusable::MomentsContent {
                    // Scroll dynamics content
                    if let Some(dynamics) = &self.selected_author_dynamics {
                        if is_down {
                            if self.dynamics_scroll_offset + 1 < dynamics.len() {
                                self.dynamics_scroll_offset += 1;
                            }
                        } else if self.dynamics_scroll_offset > 0 {
                            self.dynamics_scroll_offset -= 1;
                        }
                    }
                }
                NavigationResult::Handled
            }
            _ => NavigationResult::Continue,
        }
    }

    fn handle_horizontal_navigation(&mut self, is_right: bool) -> NavigationResult {
        // Only allow horizontal navigation in moments page
        if self.navigation.current_page != ActivePage::Moments {
            return NavigationResult::Continue;
        }

        match (self.navigation.focused_panel, is_right) {
            (Focusable::MomentsAuthors, true) => {
                // Move from authors to content
                self.set_focused_panel(Focusable::MomentsContent);
                NavigationResult::Handled
            }
            (Focusable::MomentsContent, false) => {
                // Move from content to authors
                self.set_focused_panel(Focusable::MomentsAuthors);
                NavigationResult::Handled
            }
            _ => NavigationResult::Continue,
        }
    }

    fn handle_activate(&mut self) -> NavigationResult {
        match (self.navigation.current_page, self.navigation.focused_panel) {
            (ActivePage::Search, Focusable::Search) => {
                self.set_input_mode(InputMode::Editing);
                NavigationResult::Handled
            }
            (ActivePage::Search, Focusable::Results) => {
                if self.navigation.input_mode != InputMode::ListNav {
                    self.set_input_mode(InputMode::ListNav);
                } else {
                    // In ListNav mode, Enter opens video details
                    if let Some(selected_index) = self.results_list_state.selected()
                        && let Some(video) = self.search_results.get(selected_index) {
                            // Store basic video info, will be fully loaded in detail page
                            self.video_info = Some(crate::api::VideoInfo {
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
                            self.set_active_page(ActivePage::Detail);
                            self.set_input_mode(InputMode::Normal);
                        }
                }
                NavigationResult::Handled
            }
            (ActivePage::Moments, Focusable::MomentsAuthors) => {
                if self.moments_data.as_ref().is_some_and(|d| !d.is_empty()) {
                    if self.navigation.input_mode != InputMode::ListNav {
                        self.set_input_mode(InputMode::ListNav);
                    } else {
                        // In ListNav mode, Enter switches to content panel
                        self.set_focused_panel(Focusable::MomentsContent);
                        self.set_input_mode(InputMode::Normal);
                    }
                }
                NavigationResult::Handled
            }
            (ActivePage::Moments, Focusable::MomentsContent) => {
                if self.selected_author_dynamics.is_some() {
                    self.set_input_mode(InputMode::ListNav);
                }
                NavigationResult::Handled
            }
            (ActivePage::Detail, Focusable::Search) => {
                self.set_input_mode(InputMode::Editing);
                NavigationResult::Handled
            }
            _ => NavigationResult::Continue,
        }
    }

    fn exit_list_nav_mode(&mut self) {
        self.set_input_mode(InputMode::Normal);
        match self.navigation.current_page {
            ActivePage::Search => {
                // Don't clear selection - keep the >> indicator visible
                self.set_focused_panel(Focusable::Results);
            }
            ActivePage::Moments => {
                // Don't clear selection - keep the >> indicator visible
                self.set_focused_panel(Focusable::MomentsAuthors);
            }
            _ => {}
        }
    }

    // Overlay mode handlers (simplified versions)
    async fn handle_command_mode(&mut self, key: crossterm::event::KeyEvent) -> std::io::Result<bool> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Enter => {
                let command_str = self.command_input.value().to_string();
                self.command_input.reset();
                self.overlays.command = false;

                match crate::command::parse(&command_str) {
                    Ok(crate::command::Command::Quit) => return Ok(true),
                    Ok(cmd) => {
                        if let Err(e) = crate::command::execute(cmd, self).await {
                            self.add_message(format!("Command error: {}", e), MessageLevel::Error);
                        }
                    }
                    Err(e) => {
                        self.add_message(format!("Parse error: {}", e), MessageLevel::Error);
                    }
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.overlays.command = false;
            }
            _ => {
                use tui_input::backend::crossterm::EventHandler;
                use ratatui::crossterm::event::Event;

                let ratatui_key = convert_key_event_for_input(key);
                self.command_input.handle_event(&Event::Key(ratatui_key));
            }
        }
        Ok(false)
    }

    async fn handle_help_mode(&mut self, key: crossterm::event::KeyEvent) -> std::io::Result<bool> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char(':') => {
                self.overlays.command = true;
                self.overlays.help = false;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.overlays.help = false;
            }
            KeyCode::Char('/') => {
                self.overlays.help = false;
                self.set_focused_panel(Focusable::Search);
                self.set_input_mode(InputMode::Editing);
            }
            KeyCode::Char('m') => {
                self.overlays.help = false;
                let cmd = crate::command::Command::ShowMoments;
                let _ = crate::command::execute(cmd, self).await;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_editing_mode(&mut self, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> std::io::Result<()> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Enter => {
                let query = self.search_input.value().to_string();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let response = match crate::api::search(&query).await {
                        Ok(results) => Ok(results),
                        Err(e) => Err(e.to_string()),
                    };
                    let _ = tx.send(response).await;
                });
                self.set_input_mode(InputMode::Normal);
                self.set_active_page(ActivePage::Search);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.set_input_mode(InputMode::Normal);
                self.set_focused_panel(Focusable::Search);
            }
            _ => {
                use tui_input::backend::crossterm::EventHandler;
                use ratatui::crossterm::event::Event;

                let ratatui_key = convert_key_event_for_input(key);
                self.search_input.handle_event(&Event::Key(ratatui_key));
            }
        }
        Ok(())
    }
}

