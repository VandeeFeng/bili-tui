use crate::app::{
    ActivePage, App, Focusable, InputMode, MessageLevel, NavigationAction, NavigationHandler,
    NavigationResult,
};
use ratatui::crossterm::event::KeyEvent as RatatuiKeyEvent;
use std::io;

fn convert_key_event_for_input(key: crossterm::event::KeyEvent) -> RatatuiKeyEvent {
    RatatuiKeyEvent {
        code: unsafe {
            std::mem::transmute::<crossterm::event::KeyCode, ratatui::crossterm::event::KeyCode>(
                key.code,
            )
        },
        modifiers: unsafe {
            std::mem::transmute::<
                crossterm::event::KeyModifiers,
                ratatui::crossterm::event::KeyModifiers,
            >(key.modifiers)
        },
        kind: unsafe {
            std::mem::transmute::<
                crossterm::event::KeyEventKind,
                ratatui::crossterm::event::KeyEventKind,
            >(key.kind)
        },
        state: unsafe {
            std::mem::transmute::<
                crossterm::event::KeyEventState,
                ratatui::crossterm::event::KeyEventState,
            >(key.state)
        },
    }
}

/// Main keyboard event handler - delegates to App's NavigationHandler implementation
pub async fn handle_key_event(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>,
) -> io::Result<bool> {
    app.handle_key(key, tx).await
}

/// Universal scroll key handler for popup content
fn handle_popup_scroll_keys(scroll_offset: &mut usize, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            *scroll_offset += 1;
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if *scroll_offset > 0 {
                *scroll_offset -= 1;
            }
            true
        }
        _ => false, // Not a scroll key
    }
}

impl NavigationHandler for App {
    async fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>,
    ) -> std::io::Result<bool> {
        use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }

        // Handle overlay modes first
        if self.overlays.command {
            return self.handle_command_mode(key).await;
        }

        // Handle help overlay
        if self.overlays.help {
            return self.handle_help_mode(key).await;
        }

        // Handle messages overlay
        if self.overlays.messages {
            return self.handle_messages_mode(key).await;
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
            // Content scrolling in Moments view (specific)
            KeyCode::Up
                if key.modifiers == KeyModifiers::SHIFT
                    && self.navigation.current_page == ActivePage::Moments
                    && self.navigation.focused_panel == Focusable::MomentsContent =>
            {
                NavigationAction::ContentScrollUp
            }
            KeyCode::Down
                if key.modifiers == KeyModifiers::SHIFT
                    && self.navigation.current_page == ActivePage::Moments
                    && self.navigation.focused_panel == Focusable::MomentsContent =>
            {
                NavigationAction::ContentScrollDown
            }
            KeyCode::Char('K')
                if self.navigation.current_page == ActivePage::Moments
                    && self.navigation.focused_panel == Focusable::MomentsContent =>
            {
                NavigationAction::ContentScrollUp
            }
            KeyCode::Char('J')
                if self.navigation.current_page == ActivePage::Moments
                    && self.navigation.focused_panel == Focusable::MomentsContent =>
            {
                NavigationAction::ContentScrollDown
            }

            // List navigation (general)
            KeyCode::Char('j') | KeyCode::Down if self.can_navigate_list() => {
                NavigationAction::ListDown
            }
            KeyCode::Char('k') | KeyCode::Up if self.can_navigate_list() => {
                NavigationAction::ListUp
            }
            KeyCode::Enter => NavigationAction::Activate,
            KeyCode::Char('/') => {
                self.set_focused_panel(Focusable::Search);
                self.set_input_mode(InputMode::Editing);
                return Ok(false);
            }
            KeyCode::Char('M') => NavigationAction::ToggleMessages,
            KeyCode::Char('m') => {
                // Handle moments command
                let cmd = crate::command::Command::ShowMoments;
                let _ = crate::command::execute(cmd, self).await;
                return Ok(false);
            }
            KeyCode::Char('p') => {
                // Handle play for both search results and dynamics
                if self.navigation.current_page == ActivePage::Moments
                    && self.navigation.focused_panel == Focusable::MomentsContent
                    && self.navigation.input_mode == InputMode::ListNav
                {
                    // Play video from selected dynamic
                    self.play_dynamic_video().await;
                } else {
                    // Play video from search results or detail page
                    self.play_video();
                }
                return Ok(false);
            }
            // Horizontal navigation for moments panels
            KeyCode::Char('h') | KeyCode::Left
                if self.navigation.current_page == ActivePage::Moments =>
            {
                NavigationAction::PanelLeft
            }
            KeyCode::Char('l') | KeyCode::Right
                if self.navigation.current_page == ActivePage::Moments =>
            {
                NavigationAction::PanelRight
            }
            _ => return Ok(false),
        };

        // Execute navigation action
        self.execute_navigation(action);
        Ok(false)
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
                self.overlays.help_scroll_offset = 0;
                NavigationResult::Handled
            }
            NavigationAction::ToggleMessages => {
                self.overlays.messages = true;
                self.overlays.messages_scroll_offset = 0;
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
                    self.add_message("Press Ctrl+C to exit".to_string(), MessageLevel::Info);
                    NavigationResult::Handled
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
            NavigationAction::ContentScrollDown => self.handle_content_scrolling(true),
            NavigationAction::ContentScrollUp => self.handle_content_scrolling(false),
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

    /// Calculate next index for circular navigation
    fn calculate_next_index(current: usize, max_len: usize, is_down: bool) -> usize {
        if is_down {
            if current >= max_len.saturating_sub(1) {
                0
            } else {
                current + 1
            }
        } else if current == 0 {
            max_len.saturating_sub(1)
        } else {
            current - 1
        }
    }

    /// Load dynamics for a specific author
    fn load_author_dynamics(&mut self, uid: u64) {
        // Check cache first
        if let Some(cached_dynamics) = self.author_dynamics_cache.get(&uid) {
            self.loading_dynamics = false;
            self.selected_author_dynamics = Some(cached_dynamics.clone());
            self.dynamics_scroll_offset = 0;
            self.selected_dynamic_index = 0;
            self.add_message(
                format!("Loaded {} dynamics from cache", cached_dynamics.len()),
                MessageLevel::Info,
            );
            return;
        }

        self.add_message(
            format!("Loading dynamics for UID: {}", uid),
            MessageLevel::Info,
        );
        self.loading_dynamics = true;
        self.selected_author_dynamics = None;
        self.dynamics_scroll_offset = 0;
        self.selected_dynamic_index = 0;

        // Start async loading
        if let Some(ref tx) = self.dynamics_tx {
            let tx = tx.clone();
            tokio::spawn(async move {
                let result = crate::api::get_user_dynamics(uid)
                    .await
                    .map_err(|error| error.to_string());
                let _ = tx.send((uid, result)).await;
            });
        }
    }

    /// Handle navigation in moments authors panel
    fn handle_moments_authors_navigation(&mut self, is_down: bool) -> NavigationResult {
        let Some(data) = &self.moments_data else {
            return NavigationResult::Handled;
        };

        if data.is_empty() {
            return NavigationResult::Handled;
        }

        let current = self.selected_author.selected().unwrap_or(0);
        let new_index = Self::calculate_next_index(current, data.len(), is_down);
        self.selected_author.select(Some(new_index));

        // Load dynamics for selected author
        if let Some(author) = data.get(new_index) {
            self.load_author_dynamics(author.user_profile.info.uid);
        }

        NavigationResult::Handled
    }

    /// Handle navigation in moments content panel
    fn handle_moments_content_navigation(&mut self, is_down: bool) -> NavigationResult {
        let Some(dynamics) = &self.selected_author_dynamics else {
            return NavigationResult::Handled;
        };

        if is_down {
            // Move to next dynamic
            if self.selected_dynamic_index + 1 < dynamics.len() {
                self.selected_dynamic_index += 1;
            }
        } else {
            // Move to previous dynamic
            if self.selected_dynamic_index > 0 {
                self.selected_dynamic_index -= 1;
            }
        }

        NavigationResult::Handled
    }

    /// Create VideoInfo from search result
    fn create_video_info(&self, video: &crate::api::VideoResult) -> crate::api::VideoInfo {
        crate::api::VideoInfo {
            bvid: video.bvid.clone(),
            title: video.title.clone(),
            desc: video.description.clone(),
            owner: crate::api::Owner {
                name: video.author.clone(),
            },
            stat: crate::api::Stat {
                view: video.play_count(),
                like: video.like,
                coin: 0,
                favorite: 0,
                share: 0,
            },
        }
    }

    /// Activate search panel
    fn activate_search_panel(&mut self) -> NavigationResult {
        self.set_input_mode(InputMode::Editing);
        NavigationResult::Handled
    }

    /// Activate results panel
    fn activate_results_panel(&mut self) -> NavigationResult {
        if self.navigation.input_mode != InputMode::ListNav {
            self.set_input_mode(InputMode::ListNav);
        } else {
            // In ListNav mode, Enter opens video details
            if let Some(selected_index) = self.results_list_state.selected()
                && let Some(video) = self.search_results.get(selected_index)
            {
                self.video_info = Some(self.create_video_info(video));
                self.set_active_page(ActivePage::Detail);
                self.set_input_mode(InputMode::Normal);
            }
        }
        NavigationResult::Handled
    }

    /// Activate moments authors panel
    fn activate_moments_authors(&mut self) -> NavigationResult {
        if self.moments_data.as_ref().is_none_or(|d| d.is_empty()) {
            return NavigationResult::Handled;
        }

        if self.navigation.input_mode != InputMode::ListNav {
            self.set_input_mode(InputMode::ListNav);
        } else {
            // In ListNav mode, Enter switches to content panel
            self.set_focused_panel(Focusable::MomentsContent);
            self.set_input_mode(InputMode::Normal);
        }
        NavigationResult::Handled
    }

    /// Activate moments content panel
    fn activate_moments_content(&mut self) -> NavigationResult {
        if self.selected_author_dynamics.is_some() {
            self.set_input_mode(InputMode::ListNav);
        }
        NavigationResult::Handled
    }

    /// Activate detail page search panel
    fn activate_detail_search(&mut self) -> NavigationResult {
        self.set_input_mode(InputMode::Editing);
        NavigationResult::Handled
    }

    fn handle_list_navigation(&mut self, is_down: bool) -> NavigationResult {
        match self.navigation.current_page {
            ActivePage::Search => {
                let current = self.results_list_state.selected().unwrap_or(0);
                let new_index =
                    Self::calculate_next_index(current, self.search_results.len(), is_down);
                self.results_list_state.select(Some(new_index));
                NavigationResult::Handled
            }
            ActivePage::Moments => match self.navigation.focused_panel {
                Focusable::MomentsAuthors => self.handle_moments_authors_navigation(is_down),
                Focusable::MomentsContent => self.handle_moments_content_navigation(is_down),
                _ => NavigationResult::Continue,
            },
            _ => NavigationResult::Continue,
        }
    }

    fn handle_content_scrolling(&mut self, is_down: bool) -> NavigationResult {
        if self.navigation.current_page != ActivePage::Moments
            || self.navigation.focused_panel != Focusable::MomentsContent
        {
            return NavigationResult::Continue;
        }

        if let Some(dynamics) = &self.selected_author_dynamics {
            if is_down {
                // Scroll content down
                if self.dynamics_scroll_offset + 1 < dynamics.len() {
                    self.dynamics_scroll_offset += 1;
                }
            } else {
                // Scroll content up
                if self.dynamics_scroll_offset > 0 {
                    self.dynamics_scroll_offset -= 1;
                }
            }
            self.selected_dynamic_index = self.dynamics_scroll_offset;
        }

        NavigationResult::Handled
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
            (ActivePage::Search, Focusable::Search) => self.activate_search_panel(),
            (ActivePage::Search, Focusable::Results) => self.activate_results_panel(),
            (ActivePage::Moments, Focusable::MomentsAuthors) => self.activate_moments_authors(),
            (ActivePage::Moments, Focusable::MomentsContent) => self.activate_moments_content(),
            (ActivePage::Detail, Focusable::Search) => self.activate_detail_search(),
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
    async fn handle_command_mode(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> std::io::Result<bool> {
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
                use ratatui::crossterm::event::Event;
                use tui_input::backend::crossterm::EventHandler;

                let ratatui_key = convert_key_event_for_input(key);
                self.command_input.handle_event(&Event::Key(ratatui_key));
            }
        }
        Ok(false)
    }

    async fn handle_help_mode(&mut self, key: crossterm::event::KeyEvent) -> std::io::Result<bool> {
        use crossterm::event::KeyCode;

        // Handle scrolling keys with universal function
        if handle_popup_scroll_keys(&mut self.overlays.help_scroll_offset, key) {
            return Ok(false);
        }

        match key.code {
            // Mode controls
            KeyCode::Char(':') => {
                self.overlays.command = true;
                self.overlays.help = false;
                self.overlays.help_scroll_offset = 0;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.overlays.help = false;
                self.overlays.help_scroll_offset = 0;
            }
            KeyCode::Char('/') => {
                self.overlays.help = false;
                self.overlays.help_scroll_offset = 0;
                self.set_focused_panel(Focusable::Search);
                self.set_input_mode(InputMode::Editing);
            }
            KeyCode::Char('m') => {
                self.overlays.help = false;
                self.overlays.help_scroll_offset = 0;
                let cmd = crate::command::Command::ShowMoments;
                let _ = crate::command::execute(cmd, self).await;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_messages_mode(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> std::io::Result<bool> {
        use crossterm::event::KeyCode;

        // Handle scrolling keys with universal function
        if handle_popup_scroll_keys(&mut self.overlays.messages_scroll_offset, key) {
            return Ok(false);
        }

        match key.code {
            // Mode controls
            KeyCode::Char(':') => {
                self.overlays.command = true;
                self.overlays.messages = false;
                self.overlays.messages_scroll_offset = 0;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.overlays.messages = false;
                self.overlays.messages_scroll_offset = 0;
            }
            KeyCode::Char('/') => {
                self.overlays.messages = false;
                self.overlays.messages_scroll_offset = 0;
                self.set_focused_panel(Focusable::Search);
                self.set_input_mode(InputMode::Editing);
            }
            KeyCode::Char('m') => {
                self.overlays.messages = false;
                self.overlays.messages_scroll_offset = 0;
                let cmd = crate::command::Command::ShowMoments;
                let _ = crate::command::execute(cmd, self).await;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_editing_mode(
        &mut self,
        key: crossterm::event::KeyEvent,
        tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>,
    ) -> std::io::Result<()> {
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
            KeyCode::Esc => {
                self.set_input_mode(InputMode::Normal);
                self.set_focused_panel(Focusable::Search);
            }
            _ => {
                use ratatui::crossterm::event::Event;
                use tui_input::backend::crossterm::EventHandler;

                let ratatui_key = convert_key_event_for_input(key);
                self.search_input.handle_event(&Event::Key(ratatui_key));
            }
        }
        Ok(())
    }
}
