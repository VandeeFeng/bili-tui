use crate::api;
use crate::handler::handle_key_event;
use crate::terminal;
use crate::ui;
use crossterm::event::{self, Event};
use ratatui::widgets::ListState;
use std::{error::Error, io, time::Duration};
use tui_input::Input;
use tokio::sync::mpsc;

#[derive(PartialEq, Clone, Copy)]
pub enum Focusable {
    Search,
    Results,
    None,
}

impl Focusable {
    pub fn next(self) -> Self {
        match self {
            Self::Search => Self::Results,
            Self::Results => Self::Search,
            Self::None => Self::Search,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Search => Self::Results,
            Self::Results => Self::Search,
            Self::None => Self::Results,
        }
    }
}

pub enum InputMode {
    Normal,
    Editing,
    Detail,
    ListNav,
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

    pub fn clear_messages(&mut self) {
        self.messages.clear();
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

    pub fn is_helping(&self) -> bool {
        self.help_active
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
                    self.add_message(format!("Failed to start mpv: {}", e), MessageLevel::Error);
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

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
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
            }
        };

        terminal::restore_terminal(&mut terminal)?;
        result
    }
}
