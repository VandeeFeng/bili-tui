use base64::{Engine, engine::general_purpose::STANDARD};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, MouseButton, MouseEvent, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::prelude::*;
use std::error::Error;
use std::io;
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct MessagesSelection {
    start: Option<(u16, u16)>,
    end: Option<(u16, u16)>,
    dragging: bool,
}

impl MessagesSelection {
    fn bounds(&self) -> Option<((u16, u16), (u16, u16))> {
        let (start, end) = (self.start?, self.end?);
        Some(if (start.1, start.0) <= (end.1, end.0) {
            (start, end)
        } else {
            (end, start)
        })
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let Some((start, end)) = self.bounds() else {
            return;
        };
        if !area.contains(start.into()) || !area.contains(end.into()) {
            return;
        }
        for y in start.1..=end.1 {
            let left = if y == start.1 { start.0 } else { area.x };
            let right = if y == end.1 { end.0 } else { area.right() - 1 };
            frame.buffer_mut().set_style(
                Rect::new(left, y, right - left + 1, 1),
                Style::default().add_modifier(Modifier::REVERSED),
            );
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) {
        let point = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.clear();
                if area.contains(point.into()) {
                    self.start = Some(point);
                    self.end = Some(point);
                    self.dragging = true;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                self.end = Some(clamp_point(point, area));
            }
            MouseEventKind::Up(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                self.end = Some(clamp_point(point, area));
            }
            _ => {}
        }
    }

    pub fn copy(&self, buffer: &Buffer, area: Rect) -> io::Result<()> {
        if self.dragging {
            return Ok(());
        }
        if let Some(text) = self.selected_text(buffer, area) {
            let encoded = STANDARD.encode(text);
            execute!(
                io::stdout(),
                crossterm::style::Print(format!("\u{1b}]52;c;{encoded}\u{7}"))
            )?;
        }
        Ok(())
    }

    fn selected_text(&self, buffer: &Buffer, area: Rect) -> Option<String> {
        let (start, end) = self.bounds()?;
        if !area.contains(start.into()) || !area.contains(end.into()) || start == end {
            return None;
        }
        let mut lines = Vec::new();
        for y in start.1..=end.1 {
            let mut left = if y == start.1 { start.0 } else { area.x };
            let right = if y == end.1 { end.0 } else { area.right() - 1 };
            if left > area.x
                && buffer
                    .cell((left - 1, y))
                    .is_some_and(|cell| cell.symbol().width() > 1)
            {
                left -= 1;
            }
            let line = (left..=right)
                .filter(|&x| {
                    x == area.x
                        || buffer
                            .cell((x - 1, y))
                            .is_none_or(|cell| cell.symbol().width() < 2)
                })
                .filter_map(|x| buffer.cell((x, y)))
                .map(|cell| cell.symbol())
                .collect::<String>();
            lines.push(line.trim_end().to_string());
        }
        Some(lines.join("\n"))
    }
}

fn clamp_point(point: (u16, u16), area: Rect) -> (u16, u16) {
    (
        point.0.clamp(area.x, area.right() - 1),
        point.1.clamp(area.y, area.bottom() - 1),
    )
}

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_copies_only_overlay_content() {
        let area = Rect::new(2, 1, 5, 2);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 4));
        buffer.set_string(0, 0, "outside", Style::default());
        buffer.set_string(2, 1, "hello", Style::default());
        buffer.set_string(2, 2, "world", Style::default());
        let selection = MessagesSelection {
            start: Some((4, 2)),
            end: Some((3, 1)),
            dragging: false,
        };
        assert_eq!(
            selection.selected_text(&buffer, area).as_deref(),
            Some("ello\nwor")
        );
    }

    #[test]
    fn clicks_outside_panel_do_not_select() {
        let area = Rect::new(2, 1, 5, 2);
        let mut selection = MessagesSelection::default();
        selection.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: crossterm::event::KeyModifiers::NONE,
            },
            area,
        );
        assert!(selection.bounds().is_none());
    }

    #[test]
    fn selected_text_skips_wide_character_padding() {
        let area = Rect::new(1, 1, 7, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(1, 1, "魔兽 3", Style::default());
        let selection = MessagesSelection {
            start: Some((2, 1)),
            end: Some((6, 1)),
            dragging: false,
        };
        assert_eq!(
            selection.selected_text(&buffer, area).as_deref(),
            Some("魔兽 3")
        );
    }
}
