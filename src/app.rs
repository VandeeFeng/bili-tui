use crate::api;
use ratatui::widgets::ListState;
use tui_input::Input;

#[derive(PartialEq, Clone, Copy)]
pub enum Focusable {
    Search,
    Results,
    Command,
    None,
}

impl Focusable {
    pub fn next(self) -> Self {
        match self {
            Self::Search => Self::Results,
            Self::Results => Self::Command,
            Self::Command => Self::Search,
            Self::None => Self::Search,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Search => Self::Command,
            Self::Results => Self::Search,
            Self::Command => Self::Results,
            Self::None => Self::Command,
        }
    }
}

pub enum InputMode {
    Normal,
    Editing,
    Command,
    Detail,
    ListNav,
    Help,
}

pub struct App {
    pub search_input: Input,
    pub command_input: Input,
    pub mode: InputMode,
    pub focused_panel: Focusable,
    pub search_results: Vec<api::VideoResult>,
    pub results_list_state: ListState,
    pub video_info: Option<api::VideoInfo>,
    pub last_error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            search_input: Input::default(),
            command_input: Input::default(),
            mode: InputMode::Normal,
            focused_panel: Focusable::Search,
            search_results: Vec::new(),
            results_list_state: ListState::default(),
            video_info: None,
            last_error: None,
        }
    }

    pub fn is_editing(&self) -> bool {
        matches!(self.mode, InputMode::Editing)
    }

    pub fn is_commanding(&self) -> bool {
        matches!(self.mode, InputMode::Command)
    }

    pub fn play_video(&self) {
        let bvid = if let Some(info) = &self.video_info {
            Some(info.bvid.clone())
        } else if let Some(selected) = self.results_list_state.selected() {
            self.search_results.get(selected).map(|v| v.bvid.clone())
        } else {
            None
        };

        if let Some(bvid) = bvid {
            let url = format!("https://www.bilibili.com/video/{}", bvid);
            std::process::Command::new("mpv")
                .arg(url)
                .spawn()
                .expect("failed to play video");
        }
    }
}
