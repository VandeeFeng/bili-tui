use crate::api;
use crate::config::FollowingConfig;
use crate::handler::handle_key_event;
use crate::terminal;
use crate::ui;
use crossterm::event::{self, Event};
use ratatui::widgets::ListState;
use std::{collections::HashMap, error::Error, io, time::Duration};
use tui_input::Input;
use tokio::sync::mpsc;

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
    /// Should quit application
    Quit,
    /// Continue with normal processing
    Continue,
}

/// Unified navigation handler for all keyboard and UI interactions
pub trait NavigationHandler {
    /// Handle all keyboard events
    async fn handle_key(&mut self, key: crossterm::event::KeyEvent, tx: &tokio::sync::mpsc::Sender<Result<Vec<crate::api::VideoResult>, String>>) -> std::io::Result<bool>;

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
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            command: false,
            help: false,
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

#[derive(Debug, Clone, PartialEq)]
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
    pub dynamics_tx: Option<tokio::sync::mpsc::Sender<(u64, Vec<api::AuthorDynamic>)>>,
    pub dynamics_rx: Option<tokio::sync::mpsc::Receiver<(u64, Vec<api::AuthorDynamic>)>>,
}

impl App {

    pub fn new() -> Self {
        let (dynamics_tx, dynamics_rx) = tokio::sync::mpsc::channel(32);
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
        }
    }

    pub fn add_message(&mut self, text: String, level: MessageLevel) {
        self.messages.push(Message {
            text,
            level,
        });

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
        let bvid = if let Some(info) = &self.video_info {
            Some(info.bvid.clone())
        } else if let Some(selected) = self.results_list_state.selected() {
            self.search_results.get(selected).map(|v| v.bvid.clone())
        } else {
            None
        };

        if let Some(bvid) = bvid {
            let url = format!("https://www.bilibili.com/video/{}", bvid);
            match std::process::Command::new("mpv")
                .arg("--no-terminal") // showoff mpv terminal output
                .arg(url)
                .spawn()
            {
                Ok(_) => {
                    self.add_message("Starting mpv player...".to_string(), MessageLevel::Info);
                }
                Err(e) => {
                    self.add_message(format!("Failed to start mpv: {}", e), MessageLevel::Warning);
                }
            }
        }
    }

    pub async fn play_dynamic_video(&mut self) {
        let video_title = if let Some(dynamics) = &self.selected_author_dynamics {
            if let Some(dynamic) = dynamics.get(self.selected_dynamic_index) {
                if let Some(video_info) = &dynamic.video_info {
                    Some(video_info.title.clone())
                } else {
                    self.add_message("Selected dynamic is not a video".to_string(), MessageLevel::Warning);
                    return;
                }
            } else {
                self.add_message("Invalid dynamic selection".to_string(), MessageLevel::Warning);
                return;
            }
        } else {
            self.add_message("No dynamics available".to_string(), MessageLevel::Warning);
            return;
        };

        if let Some(title) = video_title {
            self.add_message(format!("Searching for video: {}", title), MessageLevel::Info);

            match crate::api::search_video_by_title(&title).await {
                Ok(Some(bvid)) => {
                    let url = format!("https://www.bilibili.com/video/{}", bvid);
                    match std::process::Command::new("mpv")
                        .arg("--no-terminal")
                        .arg(url)
                        .spawn()
                    {
                        Ok(_) => {
                            self.add_message(format!("Playing: {}", title), MessageLevel::Success);
                        }
                        Err(e) => {
                            self.add_message(format!("Failed to start mpv: {}", e), MessageLevel::Warning);
                        }
                    }
                }
                Ok(None) => {
                    self.add_message("Video not found in search results".to_string(), MessageLevel::Warning);
                }
                Err(e) => {
                    self.add_message(format!("Search failed: {}", e), MessageLevel::Error);
                }
            }
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        let mut terminal = terminal::setup_terminal()?;
        let (tx, mut rx) = mpsc::channel(1);

        let result = loop {
            terminal.draw(|f| ui::ui(f, &mut self))?;

            if let Ok(response) = rx.try_recv() {
                match response {
                    Ok(results) => {
                        self.search_results = results;
                        if !self.search_results.is_empty() {
                            self.results_list_state.select(Some(0));
                        }
                        self.set_input_mode(InputMode::ListNav);
                        self.set_focused_panel(Focusable::Results);
                        self.set_active_page(ActivePage::Search); // Ensure we're on search page
                        self.add_message("Search completed".to_string(), MessageLevel::Success);
                    }
                    Err(e) => {
                        self.add_message(format!("Search failed: {}", e), MessageLevel::Error);
                    }
                }
            }

            // Check for dynamics loading responses
            if let Some(ref mut dynamics_rx) = self.dynamics_rx
                && let Ok((uid, dynamics)) = dynamics_rx.try_recv() {
                    let count = dynamics.len();
                    self.author_dynamics_cache.insert(uid, dynamics.clone());

                    // Update UI if this is the currently selected author
                    if let Some(selected_index) = self.selected_author.selected()
                        && let Some(ref data) = self.moments_data
                            && let Some(author) = data.get(selected_index)
                                && author.user_profile.info.uid == uid {
                                    self.selected_author_dynamics = Some(dynamics);
                                    self.loading_dynamics = false;
                                    self.dynamics_scroll_offset = 0;
                                    self.selected_dynamic_index = 0; // Reset dynamic selection
                                    self.add_message(format!("Loaded {} dynamics", count), MessageLevel::Success);
                                }
                }

            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()? {
                    match handle_key_event(&mut self, key, &tx).await {
                        Ok(should_quit) => {
                            if should_quit {
                                break Ok(());
                            }
                        }
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
}
