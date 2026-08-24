use crate::api;
use crate::config::FollowingConfig;
use crate::handler::handle_key_event;
use crate::terminal;
use crate::ui;
use crossterm::event::{self, Event};
use ratatui::widgets::ListState;
use std::{collections::HashMap, error::Error, io, time::Duration};
use tokio::sync::mpsc;
use tui_input::Input;

type DynamicsResponse = (u64, Result<Vec<api::AuthorDynamic>, String>);
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
    // Cache for author dynamics to avoid repeated API calls
    pub author_dynamics_cache: HashMap<u64, Vec<api::AuthorDynamic>>,
    // Channel to handle async dynamics loading
    pub dynamics_tx: Option<tokio::sync::mpsc::Sender<DynamicsResponse>>,
    pub dynamics_rx: Option<tokio::sync::mpsc::Receiver<DynamicsResponse>>,
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
            author_dynamics_cache: HashMap::new(),
            dynamics_tx: Some(dynamics_tx),
            dynamics_rx: Some(dynamics_rx),
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

    pub async fn play_dynamic_video(&mut self) {
        let video_title = self
            .selected_author_dynamics
            .as_ref()
            .and_then(|dynamics| dynamics.get(self.selected_dynamic_index))
            .and_then(|dynamic| dynamic.video_info.as_ref())
            .map(|video| video.title.clone());

        match video_title {
            Some(title) => {
                self.add_message(
                    format!("Searching for video: {}", title),
                    MessageLevel::Info,
                );

                match crate::api::search_video_by_title(&title).await {
                    Ok(Some(bvid)) => {
                        let url = format!("https://www.bilibili.com/video/{}", bvid);
                        self.launch_mpv(&url);
                        self.add_message(format!("Opening: {}", title), MessageLevel::Info);
                    }
                    Ok(None) => {
                        self.add_message(
                            "Video not found in search results".to_string(),
                            MessageLevel::Warning,
                        );
                    }
                    Err(e) => {
                        self.add_message(format!("Search failed: {}", e), MessageLevel::Error);
                    }
                }
            }
            None => {
                self.add_message(
                    "Selected dynamic is not a video".to_string(),
                    MessageLevel::Warning,
                );
            }
        }
    }

    pub(crate) fn launch_mpv(&mut self, url: &str) {
        let tx = self.mpv_tx.clone();
        let url = url.to_string();
        tokio::spawn(async move {
            let output = tokio::process::Command::new("mpv")
                .arg("--msg-color=no")
                .arg("--msg-level=all=error")
                .arg(url)
                .output()
                .await;
            let _ = tx.send(output).await;
        });
        self.add_message("Starting mpv player...".to_string(), MessageLevel::Info);
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        let mut terminal = terminal::setup_terminal()?;
        let (tx, mut rx) = mpsc::channel(1);

        let result = loop {
            terminal.draw(|f| ui::ui(f, &mut self))?;

            self.handle_search_response(&mut rx);
            self.handle_dynamics_response();
            self.handle_mpv_response();

            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                match handle_key_event(&mut self, key, &tx).await {
                    Ok(true) => break Ok(()),
                    Ok(false) => continue,
                    Err(e) if e.kind() == io::ErrorKind::Other && e.to_string() == "quit" => {
                        break Ok(());
                    }
                    Err(e) => break Err(e.into()),
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
                    if !self.search_results.is_empty() {
                        self.results_list_state.select(Some(0));
                    }
                    self.set_input_mode(InputMode::ListNav);
                    self.set_focused_panel(Focusable::Results);
                    self.set_active_page(ActivePage::Search);
                    self.add_message("Search completed".to_string(), MessageLevel::Success);
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
                "yt-dlp may be outdated; update it and retry".to_string(),
                MessageLevel::Warning,
            );
        }
        self.add_message(
            format!("mpv playback failed ({})", output.status),
            MessageLevel::Error,
        );
    }

    fn handle_dynamics_response(&mut self) {
        let response = self
            .dynamics_rx
            .as_mut()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some((uid, result)) = response else {
            return;
        };
        let is_selected = self.is_selected_author(uid);
        match result {
            Ok(dynamics) => {
                self.apply_dynamics(uid, dynamics);
                if is_selected {
                    self.loading_dynamics = false;
                }
            }
            Err(error) if is_selected => {
                self.loading_dynamics = false;
                self.selected_author_dynamics = None;
                self.add_message(
                    format!("Failed to load dynamics: {error}"),
                    MessageLevel::Error,
                );
            }
            Err(_) => {}
        }
    }

    fn is_selected_author(&self, uid: u64) -> bool {
        self.selected_author.selected().is_some_and(|index| {
            self.moments_data
                .as_ref()
                .and_then(|data| data.get(index))
                .is_some_and(|author| author.user_profile.info.uid == uid)
        })
    }

    fn apply_dynamics(&mut self, uid: u64, dynamics: Vec<api::AuthorDynamic>) {
        let count = dynamics.len();
        self.author_dynamics_cache.insert(uid, dynamics.clone());
        if self.is_selected_author(uid) {
            self.selected_author_dynamics = Some(dynamics);
            self.dynamics_scroll_offset = 0;
            self.selected_dynamic_index = 0;
            self.add_message(format!("Loaded {count} dynamics"), MessageLevel::Success);
        }
    }
}
