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
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertField {
    Search,
    Filter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert(InsertField),
    Interactive,
}

pub struct App {
    pub running: bool,
    pub events: EventHandler,
    pub processes: Vec<Process>,
    pub selected_index: usize,
    pub fullscreen_index: Option<usize>,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub show_help: bool,
    pub wrap: bool,
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
            input_buffer: String::new(),
            show_help: false,
            wrap: true,
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
            InputMode::Insert(_) => self.handle_insert_mode(key_event).await,
            InputMode::Interactive => self.handle_interactive_mode(key_event),
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
                    if let Some(p) = self.processes.get_mut(self.selected_index) {
                        p.scroll = 0;
                    }
                    self.fullscreen_index = None;
                }
            }
            KeyCode::Delete => {
                if let Some(p) = self.processes.get_mut(self.selected_index) {
                    p.filter = None;
                    p.search_query = None;
                    p.active_match_line = None;
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
            KeyCode::Char('w') => {
                self.wrap = !self.wrap;
                for p in &mut self.processes {
                    p.scroll = 0;
                }
            }
            KeyCode::Char('a') => {
                self.input_mode = InputMode::Insert(InsertField::Search);
                self.input_buffer.clear();
            }
            KeyCode::Char('r') => {
                self.input_mode = InputMode::Insert(InsertField::Filter);
                self.input_buffer.clear();
            }
            KeyCode::Char('g') => {
                if self.fullscreen_index.is_some()
                    && let Some(p) = self.processes.get_mut(self.selected_index)
                {
                    p.scroll = 1000;
                }
            }
            KeyCode::Char('G') => {
                if self.fullscreen_index.is_some()
                    && let Some(p) = self.processes.get_mut(self.selected_index)
                {
                    p.scroll = 0;
                }
            }
            KeyCode::Char('n') => {
                self.search_next(false);
            }
            KeyCode::Char('N') => {
                self.search_next(true);
            }
            KeyCode::Char('f') => {
                if self.fullscreen_index.is_some() {
                    if let Some(p) = self.processes.get_mut(self.selected_index) {
                        p.scroll = 0;
                    }
                    self.fullscreen_index = None;
                } else {
                    self.fullscreen_index = Some(self.selected_index);
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
            KeyCode::Char('e') => {
                let idx = self.selected_index;
                self.execute_command_on_idx(idx, "restart").await?;
            }
            KeyCode::Enter => {
                if self.fullscreen_index.is_some() {
                    if let Some(p) = self.processes.get_mut(self.selected_index) {
                        p.scroll = 0;
                    }
                    self.fullscreen_index = None;
                } else {
                    self.fullscreen_index = Some(self.selected_index);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.fullscreen_index.is_some() {
                    if let Some(p) = self.processes.get_mut(self.selected_index) {
                        p.scroll = p.scroll.saturating_add(1);
                    }
                } else if self.selected_index >= 2 {
                    self.selected_index -= 2;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.fullscreen_index.is_some() {
                    if let Some(p) = self.processes.get_mut(self.selected_index) {
                        p.scroll = p.scroll.saturating_sub(1);
                    }
                } else if self.selected_index + 2 < self.processes.len() {
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

    async fn handle_insert_mode(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                let input = self.input_buffer.clone();
                let mode = self.input_mode.clone();
                match mode {
                    InputMode::Insert(InsertField::Search) => self.apply_search(&input),
                    InputMode::Insert(InsertField::Filter) => self.apply_filter(&input),
                    _ => {}
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_interactive_mode(&mut self, key_event: KeyEvent) -> Result<()> {
        if key_event.code == KeyCode::Char('a')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.input_mode = InputMode::Normal;
            return Ok(());
        }

        if let Some(p) = self.processes.get_mut(self.selected_index) {
            let input = match key_event.code {
                KeyCode::Char(c) => {
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'a' => vec![1],
                            'b' => vec![2],
                            'c' => vec![3],
                            'd' => vec![4],
                            'e' => vec![5],
                            'f' => vec![6],
                            'g' => vec![7],
                            'h' => vec![8],
                            'i' => vec![9],
                            'j' => vec![10],
                            'k' => vec![11],
                            'l' => vec![12],
                            'm' => vec![13],
                            'n' => vec![14],
                            'o' => vec![15],
                            'p' => vec![16],
                            'q' => vec![17],
                            'r' => vec![18],
                            's' => vec![19],
                            't' => vec![20],
                            'u' => vec![21],
                            'v' => vec![22],
                            'w' => vec![23],
                            'x' => vec![24],
                            'y' => vec![25],
                            'z' => vec![26],
                            _ => vec![],
                        }
                    } else if key_event.modifiers.contains(KeyModifiers::ALT) {
                        vec![27, c as u8]
                    } else {
                        vec![c as u8]
                    }
                }
                KeyCode::Enter => vec![b'\r'],
                KeyCode::Backspace => vec![127],
                KeyCode::Tab => vec![9],
                KeyCode::Esc => vec![27],
                KeyCode::Up => vec![27, 91, 65],
                KeyCode::Down => vec![27, 91, 66],
                KeyCode::Right => vec![27, 91, 67],
                KeyCode::Left => vec![27, 91, 68],
                KeyCode::Delete => vec![27, 91, 51, 126],
                KeyCode::Home => vec![27, 72],
                KeyCode::End => vec![27, 70],
                KeyCode::PageUp => vec![27, 91, 53, 126],
                KeyCode::PageDown => vec![27, 91, 54, 126],
                KeyCode::F(1) => vec![27, 79, 80],
                KeyCode::F(2) => vec![27, 79, 81],
                KeyCode::F(3) => vec![27, 79, 82],
                KeyCode::F(4) => vec![27, 79, 83],
                KeyCode::F(5) => vec![27, 91, 49, 53, 126],
                KeyCode::F(6) => vec![27, 91, 49, 55, 126],
                KeyCode::F(7) => vec![27, 91, 49, 56, 126],
                KeyCode::F(8) => vec![27, 91, 49, 57, 126],
                KeyCode::F(9) => vec![27, 91, 50, 48, 126],
                KeyCode::F(10) => vec![27, 91, 50, 49, 126],
                KeyCode::F(11) => vec![27, 91, 50, 51, 126],
                KeyCode::F(12) => vec![27, 91, 50, 52, 126],
                _ => vec![],
            };
            if !input.is_empty() {
                p.write_input(&input)?;
            }
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
                    p.status = crate::process::ProcessStatus::Stopped;
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

    fn apply_search(&mut self, query: &str) {
        if let Some(p) = self.processes.get_mut(self.selected_index) {
            if query.trim().is_empty() {
                p.search_query = None;
                p.active_match_line = None;
            } else {
                p.search_query = Some(query.trim().to_string());
                p.active_match_line = None;
                self.search_next(false);
            }
        }
    }

    fn apply_filter(&mut self, filter: &str) {
        if let Some(p) = self.processes.get_mut(self.selected_index) {
            if filter.trim().is_empty() {
                p.filter = None;
            } else {
                p.filter = Some(filter.trim().to_string());
            }
        }
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

    fn search_next(&mut self, reverse: bool) {
        let p = match self.processes.get_mut(self.selected_index) {
            Some(p) => p,
            None => return,
        };

        let query = match &p.search_query {
            Some(q) => q.to_lowercase(),
            None => return,
        };

        let lines = p.output.lock().unwrap();
        if lines.is_empty() {
            return;
        }

        let total_lines = lines.len();
        let height = 20;

        let current_idx = p.active_match_line.unwrap_or_else(|| {
            if !reverse {
                total_lines
                    .saturating_sub(height)
                    .saturating_sub(p.scroll as usize)
            } else {
                total_lines.saturating_sub(p.scroll as usize)
            }
        });

        if !reverse {
            let start = if p.active_match_line.is_some() {
                current_idx + 1
            } else {
                current_idx
            };
            for i in start..total_lines {
                if lines[i].to_lowercase().contains(&query) {
                    p.active_match_line = Some(i);
                    p.scroll = total_lines.saturating_sub(i).saturating_sub(height / 2) as u16;
                    return;
                }
            }
            for i in 0..start {
                if lines[i].to_lowercase().contains(&query) {
                    p.active_match_line = Some(i);
                    p.scroll = total_lines.saturating_sub(i).saturating_sub(height / 2) as u16;
                    return;
                }
            }
        } else {
            let start = if p.active_match_line.is_some() {
                current_idx.saturating_sub(1)
            } else {
                current_idx
            };
            for i in (0..=start).rev() {
                if lines[i].to_lowercase().contains(&query) {
                    p.active_match_line = Some(i);
                    p.scroll = total_lines.saturating_sub(i).saturating_sub(height / 2) as u16;
                    return;
                }
            }
            for i in (start + 1..total_lines).rev() {
                if lines[i].to_lowercase().contains(&query) {
                    p.active_match_line = Some(i);
                    p.scroll = total_lines.saturating_sub(i).saturating_sub(height / 2) as u16;
                    return;
                }
            }
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
