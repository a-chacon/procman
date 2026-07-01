//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::event::{AppEvent, Event, EventHandler};
use crate::process::Process;
use crate::procfile;
use anyhow::Result;
use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::style::Color;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Interactive,
}

pub struct App {
    pub running: bool,
    pub events: EventHandler,
    pub processes: Vec<Process>,
    pub selected_index: usize,
    pub fullscreen_index: Option<usize>,
    pub input_mode: InputMode,
    pub show_help: bool,
}

const COLORS: &[Color] = &[
    Color::Cyan,
    Color::Magenta,
    Color::Yellow,
    Color::Green,
    Color::Red,
    Color::Blue,
    Color::LightCyan,
    Color::LightMagenta,
    Color::LightYellow,
    Color::LightGreen,
    Color::LightRed,
    Color::LightBlue,
];

impl App {
    pub fn new(procfile_path: String) -> Self {
        let entries = procfile::parse(&procfile_path).unwrap_or_default();
        let processes = entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| Process::new(e.name, e.command, COLORS[i % COLORS.len()]))
            .collect();

        Self {
            running: true,
            events: EventHandler::new(),
            processes,
            selected_index: 0,
            fullscreen_index: None,
            input_mode: InputMode::Normal,
            show_help: false,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        for process in &mut self.processes {
            process.spawn().await?;
        }

        while self.running {
            let area = terminal.size()?;
            self.resize_processes(area);

            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event)
                        if key_event.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        self.handle_key_events(key_event).await?
                    }
                    _ => {}
                },
                Event::App(app_event) => match app_event {
                    AppEvent::Quit => self.quit().await?,
                },
            }
        }
        Ok(())
    }

    pub async fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key_event).await,
            InputMode::Interactive => self.handle_interactive_mode(key_event).await,
        }
    }

    async fn handle_normal_mode(&mut self, key_event: KeyEvent) -> Result<()> {
        if self.show_help {
            self.show_help = false;
            return Ok(());
        }

        match key_event.code {
            KeyCode::Esc => {
                if self.fullscreen_index.is_some() {
                    self.fullscreen_index = None;
                }
            }
            KeyCode::Char('q') => self.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('p' | '?') => {
                self.show_help = true;
            }
            KeyCode::Char('i') => {
                self.input_mode = InputMode::Interactive;
            }
            KeyCode::Char('f') => {
                if self.fullscreen_index.is_some() {
                    self.fullscreen_index = None;
                } else {
                    self.fullscreen_index = Some(self.selected_index);
                }
            }
            KeyCode::PageUp => {
                if let Some(p) = self.processes.get_mut(self.selected_index) {
                    p.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(p) = self.processes.get_mut(self.selected_index) {
                    p.scroll_down(10);
                }
            }
            KeyCode::Char('u') => {
                if let Some(p) = self.processes.get_mut(self.selected_index) {
                    p.scroll_up(10);
                }
            }
            KeyCode::Char('d') => {
                if let Some(p) = self.processes.get_mut(self.selected_index) {
                    p.scroll_down(10);
                }
            }
            KeyCode::End => {
                if let Some(p) = self.processes.get_mut(self.selected_index) {
                    p.scroll_to_bottom();
                }
            }
            KeyCode::Char('s') => {
                let idx = self.selected_index;
                self.execute_command_on_idx(idx, "stop").await?;
            }
            KeyCode::Char('t') => {
                let idx = self.selected_index;
                self.execute_command_on_idx(idx, "start").await?;
            }
            KeyCode::Char('r') => {
                let idx = self.selected_index;
                self.execute_command_on_idx(idx, "restart").await?;
            }
            KeyCode::Enter => {
                if self.fullscreen_index.is_some() {
                    self.fullscreen_index = None;
                } else {
                    self.fullscreen_index = Some(self.selected_index);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index >= 2 {
                    self.selected_index -= 2;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index + 2 < self.processes.len() {
                    self.selected_index += 2;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                if self.fullscreen_index.is_some() {
                    self.fullscreen_index = Some(self.selected_index);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.selected_index + 1 < self.processes.len() {
                    self.selected_index += 1;
                }
                if self.fullscreen_index.is_some() {
                    self.fullscreen_index = Some(self.selected_index);
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let digit = c.to_digit(10).unwrap() as usize;
                if digit > 0 && digit <= self.processes.len() {
                    self.selected_index = digit - 1;
                    if self.fullscreen_index.is_some() {
                        self.fullscreen_index = Some(self.selected_index);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_interactive_mode(&mut self, key_event: KeyEvent) -> Result<()> {
        if key_event.code == KeyCode::Char('a')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.input_mode = InputMode::Normal;
            return Ok(());
        }

        if let Some(p) = self.processes.get_mut(self.selected_index) {
            if key_event.modifiers.contains(KeyModifiers::ALT) {
                match key_event.code {
                    KeyCode::PageUp => {
                        p.scroll_up(10);
                        return Ok(());
                    }
                    KeyCode::PageDown => {
                        p.scroll_down(10);
                        return Ok(());
                    }
                    KeyCode::End => {
                        p.scroll_to_bottom();
                        return Ok(());
                    }
                    _ => {}
                }
            }

            let input_bytes = match key_event.code {
                KeyCode::Char(ch) => {
                    let mut send = vec![ch as u8];
                    let upper = ch.to_ascii_uppercase();
                    if key_event.modifiers == KeyModifiers::CONTROL {
                        match upper {
                            '2' | '@' | ' ' => send = vec![0],
                            '3' | '[' => send = vec![27],
                            '4' | '\\' => send = vec![28],
                            '5' | ']' => send = vec![29],
                            '6' | '^' => send = vec![30],
                            '7' | '-' | '_' => send = vec![31],
                            char if ('A'..='_').contains(&char) => {
                                let ascii_val = char as u8;
                                let ascii_to_send = ascii_val - 64;
                                send = vec![ascii_to_send];
                            }
                            _ => {}
                        }
                    }
                    send
                }
                #[cfg(unix)]
                KeyCode::Enter => vec![b'\n'],
                #[cfg(windows)]
                KeyCode::Enter => vec![b'\r', b'\n'],
                KeyCode::Backspace => vec![8],
                KeyCode::Left => vec![27, 91, 68],
                KeyCode::Right => vec![27, 91, 67],
                KeyCode::Up => vec![27, 91, 65],
                KeyCode::Down => vec![27, 91, 66],
                KeyCode::Tab => vec![9],
                KeyCode::Home => vec![27, 91, 72],
                KeyCode::End => vec![27, 91, 70],
                KeyCode::PageUp => vec![27, 91, 53, 126],
                KeyCode::PageDown => vec![27, 91, 54, 126],
                KeyCode::BackTab => vec![27, 91, 90],
                KeyCode::Delete => vec![27, 91, 51, 126],
                KeyCode::Insert => vec![27, 91, 50, 126],
                KeyCode::Esc => vec![27],
                _ => return Ok(()),
            };

            p.write_input(Bytes::from(input_bytes)).await?;
        }
        Ok(())
    }

    async fn execute_command_on_idx(&mut self, idx: usize, command: &str) -> Result<()> {
        if let Some(p) = self.processes.get_mut(idx) {
            match command {
                "start" => {
                    if p.status != crate::process::ProcessStatus::Running {
                        p.spawn().await?;
                    }
                }
                "stop" => {
                    p.kill().await?;
                }
                "restart" => {
                    p.kill().await?;
                    p.spawn().await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn resize_processes(&mut self, size: ratatui::layout::Size) {
        if let Some(idx) = self.fullscreen_index {
            if let Some(p) = self.processes.get_mut(idx) {
                let _ = p.resize_pty(size.height.saturating_sub(5), size.width.saturating_sub(2));
            }
            return;
        }

        let num_processes = self.processes.len();
        if num_processes == 0 {
            return;
        }
        let num_cols = 2;
        let num_rows = num_processes.div_ceil(num_cols);

        let cell_height = size.height / num_rows as u16;
        let cell_width = size.width / num_cols as u16;

        for p in &mut self.processes {
            let _ = p.resize_pty(cell_height.saturating_sub(2), cell_width.saturating_sub(2));
        }
    }

    pub fn tick(&self) {}

    pub async fn quit(&mut self) -> Result<()> {
        for process in &mut self.processes {
            process.kill().await?;
        }
        self.running = false;
        Ok(())
    }
}
