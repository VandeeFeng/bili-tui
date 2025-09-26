mod api;
mod app;
mod command;
mod ui;

use app::{App, Focusable, InputMode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::{error::Error, io, time::Duration};
use tokio::sync::mpsc;
use tui_input::backend::crossterm::EventHandler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let (tx, mut rx) = mpsc::channel(1);

    loop {
        terminal.draw(|f| ui::ui(f, &mut app))?;

        if let Ok(response) = rx.try_recv() {
            match response {
                Ok(results) => {
                    app.search_results = results;
                    if !app.search_results.is_empty() {
                        app.results_list_state.select(Some(0));
                    }
                    app.mode = InputMode::ListNav;
                    app.focused_panel = Focusable::Results;
                    app.last_error = None;
                }
                Err(e) => {
                    app.last_error = Some(e);
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => {
                                return Ok(());
                            }
                            KeyCode::Char('j') => {
                                app.focused_panel = app.focused_panel.next();
                            }
                            KeyCode::Char('k') => {
                                app.focused_panel = app.focused_panel.prev();
                            }
                            KeyCode::Enter => match app.focused_panel {
                                Focusable::Search => {
                                    app.mode = InputMode::Editing;
                                }
                                Focusable::Results => {
                                    app.mode = InputMode::ListNav;
                                }
                                Focusable::Command => {
                                    app.mode = InputMode::Command;
                                }
                                Focusable::None => {}
                            },
                            KeyCode::Char(':') => {
                                if app.focused_panel == Focusable::Command {
                                    app.mode = InputMode::Command;
                                    app.command_input.reset();
                                    app.command_input.handle_event(&Event::Key(key));
                                }
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                let query = app.search_input.value().to_string();
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    let response = match api::search(&query).await {
                                        Ok(results) => Ok(results),
                                        Err(e) => Err(e.to_string()),
                                    };
                                    let _ = tx.send(response).await;
                                });
                                app.mode = InputMode::Normal;
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.mode = InputMode::Normal;
                                app.focused_panel = Focusable::None;
                            }
                            _ => {
                                app.search_input.handle_event(&Event::Key(key));
                            }
                        },
                        InputMode::Command => match key.code {
                            KeyCode::Enter => {
                                let command_str = app.command_input.value().to_string();
                                app.command_input.reset();
                                app.mode = InputMode::Normal;

                                match command::parse(&command_str) {
                                    Ok(cmd) => {
                                        if let command::Command::Quit = cmd {
                                            return Ok(());
                                        }
                                        match command::execute(cmd, &mut app).await {
                                            Ok(_) => app.last_error = None,
                                            Err(e) => app.last_error = Some(e),
                                        }
                                    }
                                    Err(e) => {
                                        app.last_error = Some(e);
                                    }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.mode = InputMode::Normal;
                                app.focused_panel = Focusable::None;
                            }
                            _ => {
                                app.command_input.handle_event(&Event::Key(key));
                            }
                        },
                        InputMode::Detail => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.mode = InputMode::ListNav;
                            }
                            KeyCode::Char('p') => {
                                app.play_video();
                            }
                            _ => {}
                        },
                        InputMode::ListNav => match key.code {
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
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.mode = InputMode::Normal;
                                app.results_list_state.select(None);
                                app.focused_panel = Focusable::None;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }
}
