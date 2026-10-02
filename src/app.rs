use crate::api;
use crate::config::FollowingConfig;
use crate::handler::handle_key_event;
use crate::terminal;
use crate::ui;
use crossterm::event::{self, Event};
use ratatui::widgets::ListState;
use std::{
    collections::HashMap,
    error::Error,
    io,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tui_input::Input;

type DynamicsResponse = (u64, Result<api::DynamicsLoad, String>);

fn is_bilibili_url(url: &str) -> bool {
    url::Url::parse(url).ok().is_some_and(|parsed| {
        parsed
            .host_str()
            .is_some_and(|host| host == "bilibili.com" || host.ends_with(".bilibili.com"))
    })
}

type MpvResponse = std::io::Result<std::process::Output>;

#[derive(PartialEq, Clone, Copy)]
pub enum Focusable {
    Search,
    Results,
    MomentsAuthors,
    MomentsContent,
}

/// Unified navigation actions
#[derive(PartialEq, Clone, Copy)]
pub enum NavigationAction {
    PanelNext,
    PanelPrev,
    ListUp,
    ListDown,
    Activate,
    Exit,
    ToggleCommand,
    ToggleHelp,
    ToggleMessages,
    PanelLeft,
    PanelRight,
    ContentScrollUp,
    ContentScrollDown,
}

/// Result of handling navigation actions
#[derive(Debug, PartialEq)]
pub enum NavigationResult {
    /// Action was handled
    Handled,
    /// Continue with normal processing
    Continue,
}

/// Unified navigation handler for all keyboard and UI interactions
pub trait NavigationHandler {
    /// Handle all keyboard events
    async fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>,
    ) -> std::io::Result<bool>;

    /// Execute navigation action
    fn execute_navigation(&mut self, action: NavigationAction) -> NavigationResult;

    /// Check if panel navigation is allowed
    fn can_navigate_panels(&self) -> bool;

    /// Check if list navigation is allowed
    fn can_navigate_list(&self) -> bool;
}

impl Focusable {
    pub fn next(self) -> Self {
        match self {
            Self::Search => Self::Results,
            Self::Results => Self::MomentsAuthors,
            Self::MomentsAuthors => Self::MomentsContent,
            Self::MomentsContent => Self::Search,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Search => Self::MomentsContent,
            Self::Results => Self::Search,
            Self::MomentsAuthors => Self::Results,
            Self::MomentsContent => Self::MomentsAuthors,
        }
    }
}

// Page state - manages currently displayed page
#[derive(PartialEq, Clone, Copy)]
pub enum ActivePage {
    Search,
    Moments,
    Detail,
}

// Input mode - handles current interaction method only
#[derive(PartialEq, Clone)]
pub enum InputMode {
    Normal,
    Editing,
    ListNav,
}

// Unified overlay state management
#[derive(PartialEq, Clone)]
pub struct OverlayState {
    pub command: bool,
    pub help: bool,
    pub messages: bool,
    pub help_scroll_offset: usize,
    pub messages_scroll_offset: usize,
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            command: false,
            help: false,
            messages: false,
            help_scroll_offset: 0,
            messages_scroll_offset: 0,
        }
    }
}

// Unified navigation state
#[derive(PartialEq, Clone)]
pub struct NavigationState {
    pub current_page: ActivePage,
    pub input_mode: InputMode,
    pub focused_panel: Focusable,
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            current_page: ActivePage::Search,
            input_mode: InputMode::Normal,
            focused_panel: Focusable::Search,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub level: MessageLevel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
}

pub struct App {
    // Input fields
    pub search_input: Input,
    pub command_input: Input,

    // State management
    pub navigation: NavigationState,
    pub overlays: OverlayState,

    // Data
    pub search_results: Vec<api::VideoResult>,
    pub results_list_state: ListState,
    pub video_info: Option<api::VideoInfo>,
    pub last_error: Option<String>,
    pub messages: Vec<Message>,
    pub show_error_popup: bool,

    // Config
    pub following_config: FollowingConfig,

    // Moments related fields
    pub moments_data: Option<Vec<api::AuthorItem>>,
    pub selected_author: ListState,
    pub selected_author_dynamics: Option<Vec<api::AuthorDynamic>>,
    pub loading_dynamics: bool,
    pub dynamics_scroll_offset: usize,
    pub selected_dynamic_index: usize,
    pub dynamics_viewport_height: usize,
    pub(crate) loading_author_uid: Option<u64>,
    pub(crate) dynamics_cache: HashMap<u64, (Instant, Vec<api::AuthorDynamic>)>,
    pub(crate) scheduled_dynamics: Option<(u64, Instant)>,
    pub(crate) last_manual_dynamics_refresh: Option<Instant>,
    pub(crate) dynamics_tx: tokio::sync::mpsc::Sender<DynamicsResponse>,
    dynamics_rx: tokio::sync::mpsc::Receiver<DynamicsResponse>,
    mpv_tx: tokio::sync::mpsc::Sender<MpvResponse>,
    mpv_rx: tokio::sync::mpsc::Receiver<MpvResponse>,
}

impl App {
    pub fn new() -> Self {
        let (dynamics_tx, dynamics_rx) = tokio::sync::mpsc::channel(32);
        let (mpv_tx, mpv_rx) = tokio::sync::mpsc::channel(4);
        let following_config = FollowingConfig::load().unwrap_or_default();
        Self {
            search_input: Input::default(),
            command_input: Input::default(),
            navigation: NavigationState::new(),
            overlays: OverlayState::new(),
            search_results: Vec::new(),
            results_list_state: ListState::default(),
            video_info: None,
            last_error: None,
            messages: Vec::new(),
            show_error_popup: false,
            following_config,
            // Moments related fields
            moments_data: None,
            selected_author: ListState::default(),
            selected_author_dynamics: None,
            loading_dynamics: false,
            dynamics_scroll_offset: 0,
            selected_dynamic_index: 0,
            dynamics_viewport_height: 20, // Default value
            loading_author_uid: None,
            dynamics_cache: HashMap::new(),
            scheduled_dynamics: None,
            last_manual_dynamics_refresh: None,
            dynamics_tx,
            dynamics_rx,
            mpv_tx,
            mpv_rx,
        }
    }

    pub fn add_message(&mut self, text: String, level: MessageLevel) {
        self.messages.push(Message { text, level });

        // Keep only last 50 messages
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
    }

    pub fn get_latest_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    pub fn is_editing(&self) -> bool {
        matches!(self.navigation.input_mode, InputMode::Editing)
    }

    pub fn is_commanding(&self) -> bool {
        self.overlays.command
    }

    pub fn active_page(&self) -> ActivePage {
        self.navigation.current_page
    }

    pub fn focused_panel(&self) -> Focusable {
        self.navigation.focused_panel
    }

    pub fn input_mode(&self) -> InputMode {
        self.navigation.input_mode.clone()
    }

    pub fn set_active_page(&mut self, page: ActivePage) {
        self.navigation.current_page = page;
    }

    pub fn set_focused_panel(&mut self, panel: Focusable) {
        self.navigation.focused_panel = panel;
    }

    pub fn set_input_mode(&mut self, mode: InputMode) {
        self.navigation.input_mode = mode;
    }

    pub fn play_video(&mut self) {
        let bvid = self
            .video_info
            .as_ref()
            .map(|info| info.bvid.clone())
            .or_else(|| {
                self.results_list_state
                    .selected()
                    .and_then(|idx| self.search_results.get(idx).map(|v| v.bvid.clone()))
            });

        match bvid {
            Some(bvid) => {
                let url = format!("https://www.bilibili.com/video/{}", bvid);
                self.launch_mpv(&url);
            }
            None => {
                self.add_message("No video selected".to_string(), MessageLevel::Warning);
            }
        }
    }

    pub fn play_dynamic_video(&mut self) {
        let video = self
            .selected_author_dynamics
            .as_ref()
            .and_then(|dynamics| dynamics.get(self.selected_dynamic_index))
            .and_then(|dynamic| dynamic.video_info.as_ref());

        if let Some(video) = video {
            let url = format!("https://www.bilibili.com/video/{}", video.bvid);
            let title = video.title.clone();
            self.launch_mpv(&url);
            self.add_message(format!("Opening: {title}"), MessageLevel::Info);
        } else {
            self.add_message(
                "Selected dynamic is not a video".to_string(),
                MessageLevel::Warning,
            );
        }
    }

    pub(crate) fn launch_mpv(&mut self, url: &str) {
        let tx = self.mpv_tx.clone();
        let url = url.to_string();
        tokio::spawn(async move {
            let mut command = tokio::process::Command::new("mpv");
            command.arg("--msg-color=no").arg("--msg-level=all=error");
            if is_bilibili_url(&url)
                && let Ok(sessdata) = std::env::var("BILI_SESSDATA")
                && !sessdata.is_empty()
            {
                command.arg(format!(
                    "--ytdl-raw-options=add-headers=Cookie: SESSDATA={sessdata}"
                ));
            }
            let output = command.arg(url).output().await;
            let _ = tx.send(output).await;
        });
        self.add_message("Starting mpv player...".to_string(), MessageLevel::Info);
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        let mut terminal = terminal::setup_terminal()?;
        let (tx, mut rx) = mpsc::channel(1);
        let mut selection = terminal::MessagesSelection::default();
        let mut rendered_messages = None;
        let mut rendered_moments = None;
        let mut hovering_link = false;

        let result = loop {
            terminal.draw(|f| {
                ui::ui(f, &mut self);
                if self.overlays.messages {
                    let area = ui::components::popups::messages_content_area(f.area(), &self);
                    selection.draw(f, area);
                    rendered_messages = Some(f.buffer_mut().clone());
                } else {
                    rendered_messages = None;
                }
                rendered_moments = (self.active_page() == ActivePage::Moments
                    && !self.overlays.messages
                    && !self.overlays.help
                    && !self.is_commanding()
                    && !self.show_error_popup)
                    .then(|| f.buffer_mut().clone());
            })?;
            if rendered_moments.is_none() && hovering_link {
                terminal::set_link_hover(false)?;
                hovering_link = false;
            }

            self.handle_search_response(&mut rx);
            self.handle_dynamics_response();
            self.handle_mpv_response();

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Mouse(mouse) if self.overlays.messages => {
                        if let Some(buffer) = &rendered_messages {
                            let area =
                                ui::components::popups::messages_content_area(buffer.area, &self);
                            if area.width > 0 && area.height > 0 {
                                selection.handle_mouse(mouse, area);
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        let url = rendered_moments.as_ref().and_then(|buffer| {
                            terminal::http_url_at(buffer, mouse.column, mouse.row)
                        });
                        if mouse.kind == crossterm::event::MouseEventKind::Moved
                            && hovering_link != url.is_some()
                        {
                            hovering_link = url.is_some();
                            terminal::set_link_hover(hovering_link)?;
                        }
                        if mouse.kind
                            == crossterm::event::MouseEventKind::Down(
                                crossterm::event::MouseButton::Left,
                            )
                            && mouse
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            && let Some(url) = url
                            && let Err(error) =
                                std::process::Command::new("xdg-open").arg(url).spawn()
                        {
                            self.add_message(
                                format!("Failed to open URL: {error}"),
                                MessageLevel::Error,
                            );
                        }
                    }
                    Event::Key(key)
                        if self.overlays.messages
                            && key.code == crossterm::event::KeyCode::Char('y')
                            && key.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        if let Some(buffer) = &rendered_messages {
                            let area =
                                ui::components::popups::messages_content_area(buffer.area, &self);
                            selection.copy(buffer, area)?;
                        }
                    }
                    Event::Key(key) => {
                        match handle_key_event(&mut self, key, &tx).await {
                            Ok(true) => break Ok(()),
                            Ok(false) => {}
                            Err(e)
                                if e.kind() == io::ErrorKind::Other && e.to_string() == "quit" =>
                            {
                                break Ok(());
                            }
                            Err(e) => break Err(e.into()),
                        }
                        selection.clear();
                    }
                    _ => {}
                }
            }
        };

        terminal::restore_terminal(&mut terminal)?;
        result
    }

    fn handle_search_response(
        &mut self,
        rx: &mut mpsc::Receiver<Result<Vec<crate::api::VideoResult>, String>>,
    ) {
        if let Ok(response) = rx.try_recv() {
            match response {
                Ok(results) => {
                    self.search_results = results;
                    if self.search_results.is_empty() {
                        self.results_list_state.select(None);
                        self.set_input_mode(InputMode::Normal);
                        self.set_focused_panel(Focusable::Search);
                        self.add_message("No videos found".to_string(), MessageLevel::Warning);
                    } else {
                        self.results_list_state.select(Some(0));
                        self.set_input_mode(InputMode::ListNav);
                        self.set_focused_panel(Focusable::Results);
                        self.add_message(
                            format!("Found {} videos", self.search_results.len()),
                            MessageLevel::Success,
                        );
                    }
                    self.set_active_page(ActivePage::Search);
                }
                Err(e) => {
                    self.add_message(format!("Search failed: {}", e), MessageLevel::Error);
                }
            }
        }
    }

    fn handle_mpv_response(&mut self) {
        let Ok(response) = self.mpv_rx.try_recv() else {
            return;
        };
        let output = match response {
            Ok(output) if output.status.success() => {
                self.add_message("Playback finished".to_string(), MessageLevel::Success);
                return;
            }
            Ok(output) => output,
            Err(error) => {
                self.add_message(format!("Failed to start mpv: {error}"), MessageLevel::Error);
                return;
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stdout.lines().chain(stderr.lines()) {
            self.add_message(line.to_string(), MessageLevel::Error);
        }
        if stderr.contains("HTTP Error 412") || stdout.contains("HTTP Error 412") {
            self.add_message(
                "Bilibili rejected the request (HTTP 412); browser cookies may be required"
                    .to_string(),
                MessageLevel::Warning,
            );
        }
        self.add_message(
            format!("mpv playback failed ({})", output.status),
            MessageLevel::Error,
        );
    }

    fn handle_dynamics_response(&mut self) {
        self.start_scheduled_dynamics();
        let Ok((uid, result)) = self.dynamics_rx.try_recv() else {
            return;
        };
        self.loading_author_uid = None;
        let is_selected = self.selected_author_uid() == Some(uid);
        match result {
            Ok(loaded) => {
                if is_selected && let Some(warning) = loaded.warning {
                    self.add_message(warning, MessageLevel::Warning);
                }
                let dynamics = loaded.items;
                let count = dynamics.len();
                self.dynamics_cache
                    .insert(uid, (Instant::now(), dynamics.clone()));
                if is_selected {
                    self.apply_dynamics(dynamics);
                    self.add_message(format!("Loaded {count} dynamics"), MessageLevel::Success);
                }
            }
            Err(error) if is_selected => {
                self.loading_dynamics = false;
                self.add_message(
                    format!("Failed to load dynamics: {error}"),
                    MessageLevel::Error,
                );
            }
            Err(_) => {}
        }
    }

    pub(crate) fn selected_author_uid(&self) -> Option<u64> {
        let index = self.selected_author.selected()?;
        Some(
            self.moments_data
                .as_ref()?
                .get(index)?
                .user_profile
                .info
                .uid,
        )
    }

    pub(crate) fn apply_dynamics(&mut self, dynamics: Vec<api::AuthorDynamic>) {
        self.selected_author_dynamics = Some(dynamics);
        self.dynamics_scroll_offset = 0;
        self.selected_dynamic_index = 0;
        self.loading_dynamics = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{App, is_bilibili_url};
    use std::time::{Duration, Instant};

    fn moments_app() -> App {
        let mut app = App::new();
        app.moments_data = Some(vec![
            serde_json::from_str(
                r#"{"user_profile":{"info":{"uid":18446744073709551615,"uname":"test"}}}"#,
            )
            .unwrap(),
        ]);
        app.selected_author.select(Some(0));
        app
    }

    #[test]
    fn dynamics_cache_and_selection_do_not_duplicate_requests() {
        let mut app = moments_app();
        app.dynamics_cache
            .insert(u64::MAX, (Instant::now(), vec![]));
        for _ in 0..2 {
            app.load_author_dynamics(u64::MAX);
        }
        assert!(app.selected_author_dynamics.as_ref().unwrap().is_empty());
        assert!(!app.loading_dynamics);
        assert!(app.scheduled_dynamics.is_none());
        app.dynamics_cache.clear();
        app.scheduled_dynamics = Some((123, Instant::now()));
        app.load_author_dynamics(u64::MAX);
        assert_eq!(app.scheduled_dynamics.unwrap().0, u64::MAX);
        app.start_scheduled_dynamics();
        assert!(app.loading_author_uid.is_none());
        app.loading_author_uid = Some(123);
        app.scheduled_dynamics.as_mut().unwrap().1 -= Duration::from_secs(1);
        app.start_scheduled_dynamics();
        assert_eq!(app.loading_author_uid, Some(123));
        app.loading_author_uid = Some(u64::MAX);
        app.load_author_dynamics(u64::MAX);
        assert!(app.scheduled_dynamics.is_none());
        assert!(app.messages.is_empty());
    }

    #[test]
    fn dynamics_responses_cache_results_and_preserve_selection_on_failure() {
        let mut app = moments_app();
        let items: Vec<crate::api::AuthorDynamic> = serde_json::from_str(
            r#"[{"content":"fresh","timestamp":1,"author_name":"test","stats":null,"video_info":null}]"#,
        ).unwrap();
        app.selected_author_dynamics = Some(vec![]);
        for uid in [123, u64::MAX] {
            app.loading_author_uid = Some(uid);
            app.loading_dynamics = true;
            app.dynamics_tx
                .try_send((
                    uid,
                    Ok(crate::api::DynamicsLoad {
                        items: items.clone(),
                        warning: None,
                    }),
                ))
                .unwrap();
            app.handle_dynamics_response();
            assert_eq!(app.dynamics_cache[&uid].1[0].content, "fresh");
            assert!(app.loading_author_uid.is_none());
            if uid == 123 {
                assert!(app.selected_author_dynamics.as_ref().unwrap().is_empty());
                assert!(app.messages.is_empty());
            } else {
                assert!(!app.loading_dynamics);
                assert_eq!(
                    app.selected_author_dynamics.as_ref().unwrap()[0].content,
                    "fresh"
                );
            }
        }
        app.loading_author_uid = Some(u64::MAX);
        app.loading_dynamics = true;
        app.dynamics_tx
            .try_send((u64::MAX, Err("network error".to_string())))
            .unwrap();
        app.handle_dynamics_response();
        assert!(!app.loading_dynamics);
        assert_eq!(
            app.selected_author_dynamics.as_ref().unwrap()[0].content,
            "fresh"
        );
        assert!(app.loading_author_uid.is_none());
    }

    #[tokio::test]
    async fn refresh_key_bypasses_cache_only_in_moments_and_obeys_global_cooldown() {
        use super::{ActivePage, Focusable, InputMode};
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        let mut app = moments_app();
        app.set_input_mode(InputMode::Normal);
        crate::handler::handle_key_event(&mut app, key, &tx)
            .await
            .unwrap();
        assert!(app.loading_author_uid.is_none());
        app.set_active_page(ActivePage::Moments);
        app.set_focused_panel(Focusable::MomentsAuthors);
        app.dynamics_cache
            .insert(u64::MAX, (Instant::now(), vec![]));
        app.selected_author_dynamics = Some(vec![]);
        app.loading_author_uid = Some(123);
        app.refresh_selected_author_dynamics();
        assert!(app.last_manual_dynamics_refresh.is_none());
        app.loading_author_uid = None;
        crate::handler::handle_key_event(&mut app, key, &tx)
            .await
            .unwrap();
        assert_eq!(app.loading_author_uid, Some(u64::MAX));
        assert!(app.selected_author_dynamics.is_some());
        let refreshed_at = app.last_manual_dynamics_refresh;
        app.loading_author_uid = None;
        app.moments_data.as_mut().unwrap()[0].user_profile.info.uid = 123;
        app.set_focused_panel(Focusable::MomentsContent);
        crate::handler::handle_key_event(&mut app, key, &tx)
            .await
            .unwrap();
        assert!(app.loading_author_uid.is_none());
        assert_eq!(app.last_manual_dynamics_refresh, refreshed_at);
        app.last_manual_dynamics_refresh = Some(Instant::now() - Duration::from_secs(60));
        crate::handler::handle_key_event(&mut app, key, &tx)
            .await
            .unwrap();
        assert_eq!(app.loading_author_uid, Some(123));
    }

    #[test]
    fn only_bilibili_hosts_receive_session_cookie() {
        assert!(is_bilibili_url("https://www.bilibili.com/video/BV1example"));
        assert!(!is_bilibili_url(
            "https://bilibili.com.evil.test/video/BV1example"
        ));
        assert!(!is_bilibili_url("https://other.example/video/BV1example"));
    }
}
