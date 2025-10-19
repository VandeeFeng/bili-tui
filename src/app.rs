use crate::api;
use crate::handler::handle_key_event;
use crate::terminal;
use crate::ui;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::ListState;
use std::{error::Error, io, time::Duration};
use tui_input::Input;
use tokio::sync::mpsc;

#[derive(PartialEq, Clone, Copy)]
pub enum Focusable {
    Search,
    Results,
    MomentsAuthors,
    MomentsContent,
    None,
}

/// Trait for handling common keyboard event patterns across different modes
pub trait StateHandler {
    /// Handle common navigation keys (j/k, :, ?, q/Esc)
    fn handle_common_keys(&mut self, key: crossterm::event::KeyEvent) -> CommonKeyResult;

    /// Toggle command mode
    fn activate_command(&mut self);

    /// Toggle help mode
    fn activate_help(&mut self);
}

/// Result of handling common keys
#[derive(Debug, PartialEq)]
pub enum CommonKeyResult {
    /// Key was handled as a common action
    Handled,
    /// Key should be processed by mode-specific logic
    Continue,
    /// Application should quit
    Quit,
}

impl StateHandler for App {
    fn handle_common_keys(&mut self, key: crossterm::event::KeyEvent) -> CommonKeyResult {
        match key.code {
            KeyCode::Char(':') => {
                self.activate_command();
                CommonKeyResult::Handled
            }
            KeyCode::Char('?') => {
                self.activate_help();
                CommonKeyResult::Handled
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                CommonKeyResult::Quit
            }
            _ => CommonKeyResult::Continue,
        }
    }

    fn activate_command(&mut self) {
        self.command_active = true;
        self.command_input.reset();
    }

    fn activate_help(&mut self) {
        self.help_active = true;
    }
}

/// Trait for focus navigation
pub trait FocusNavigation {
    fn move_focus_next(&mut self);
    fn move_focus_prev(&mut self);
}

impl FocusNavigation for App {
    fn move_focus_next(&mut self) {
        self.focused_panel = self.focused_panel.next();
    }

    fn move_focus_prev(&mut self) {
        self.focused_panel = self.focused_panel.prev();
    }
}

impl Focusable {
    pub fn next(self) -> Self {
        match self {
            Self::Search => Self::Results,
            Self::Results => Self::MomentsAuthors,
            Self::MomentsAuthors => Self::MomentsContent,
            Self::MomentsContent => Self::Search,
            Self::None => Self::Search,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Search => Self::MomentsContent,
            Self::Results => Self::Search,
            Self::MomentsAuthors => Self::Results,
            Self::MomentsContent => Self::MomentsAuthors,
            Self::None => Self::Results,
        }
    }
}

pub enum InputMode {
    Normal,
    Editing,
    Detail,
    ListNav,
    Moments,
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
    pub search_input: Input,
    pub command_input: Input,
    pub mode: InputMode,
    pub command_active: bool,
    pub help_active: bool,
    pub focused_panel: Focusable,
    pub search_results: Vec<api::VideoResult>,
    pub results_list_state: ListState,
    pub video_info: Option<api::VideoInfo>,
    pub last_error: Option<String>,
    pub messages: Vec<Message>,
    pub show_error_popup: bool,
    // Moments related fields
    pub moments_active: bool,
    pub moments_data: Option<Vec<api::AuthorItem>>,
    pub selected_author: ListState,
    pub selected_author_dynamics: Option<Vec<api::AuthorDynamic>>,
    pub loading_dynamics: bool,
    pub dynamics_scroll_offset: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            search_input: Input::default(),
            command_input: Input::default(),
            mode: InputMode::Normal,
            command_active: false,
            help_active: false,
            focused_panel: Focusable::Search,
            search_results: Vec::new(),
            results_list_state: ListState::default(),
            video_info: None,
            last_error: None,
            messages: Vec::new(),
            show_error_popup: false,
            // Moments related fields
            moments_active: false,
            moments_data: None,
            selected_author: ListState::default(),
            selected_author_dynamics: None,
            loading_dynamics: false,
            dynamics_scroll_offset: 0,
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
        matches!(self.mode, InputMode::Editing)
    }

    pub fn is_commanding(&self) -> bool {
        self.command_active
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
                        self.mode = InputMode::ListNav;
                        self.focused_panel = Focusable::Results;
                        self.add_message("Search completed".to_string(), MessageLevel::Success);
                    }
                    Err(e) => {
                        self.add_message(format!("Search failed: {}", e), MessageLevel::Error);
                    }
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
