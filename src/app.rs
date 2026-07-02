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
use ratatui::layout::Size;
use ratatui::style::Color;
use std::time::Instant;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownStatus {
    Pending,
    Stopping,
    Done,
}

pub struct App {
    pub running: bool,
    pub events: EventHandler,
    pub processes: Vec<Process>,
    pub selected_index: usize,
    pub input_mode: InputMode,
    pub show_help: bool,
    pub last_terminal_size: Option<Size>,
    pub shutdown_states: Option<Vec<ShutdownStatus>>,
    pub system: System,
    pub last_stats_refresh: Instant,
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
            input_mode: InputMode::Normal,
            show_help: false,
            last_terminal_size: None,
            shutdown_states: None,
            system: System::new_all(),
            last_stats_refresh: Instant::now(),
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let initial_area = terminal.size()?;
        self.last_terminal_size = Some(initial_area);
        let (rows, cols) = Self::pty_dimensions(initial_area);

        for process in &mut self.processes {
            process.spawn_with_size(rows, cols).await?;
        }

        while self.running {
            let area = terminal.size()?;
            self.last_terminal_size = Some(area);
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
                    AppEvent::Quit => self.shutdown_with_modal(&mut terminal).await?,
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
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Left | KeyCode::Char('h') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Right | KeyCode::Char('l') => {
                if self.selected_index + 1 < self.processes.len() {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let digit = c.to_digit(10).unwrap() as usize;
                if digit > 0 && digit <= self.processes.len() {
                    self.selected_index = digit - 1;
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
        let (rows, cols) = self.current_pty_dimensions();

        if let Some(p) = self.processes.get_mut(idx) {
            match command {
                "start" => {
                    if p.status != crate::process::ProcessStatus::Running {
                        p.spawn_with_size(rows, cols).await?;
                    }
                }
                "stop" => {
                    p.kill().await?;
                }
                "restart" => {
                    p.kill().await?;
                    p.spawn_with_size(rows, cols).await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn resize_processes(&mut self, size: ratatui::layout::Size) {
        if self.processes.is_empty() {
            return;
        }

        let (rows, cols) = Self::pty_dimensions(size);
        if let Some(p) = self.processes.get_mut(self.selected_index) {
            let _ = p.resize_pty(rows, cols);
        }
    }

    fn current_pty_dimensions(&self) -> (u16, u16) {
        match self.last_terminal_size {
            Some(size) => Self::pty_dimensions(size),
            None => (24, 80),
        }
    }

    fn pty_dimensions(size: Size) -> (u16, u16) {
        (
            size.height.saturating_sub(5).max(1),
            size.width.saturating_sub(2).max(1),
        )
    }

    pub fn tick(&mut self) {
        // Refresh system stats at a lower rate than render ticks to avoid UI stalls.
        // Tick runs at 30 FPS, but process memory does not need frame-level updates.
        if self.last_stats_refresh.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_stats_refresh = Instant::now();

        let pids: Vec<Pid> = self
            .processes
            .iter()
            .filter_map(|p| p.process_id.map(|pid| Pid::from(pid as usize)))
            .collect();

        if !pids.is_empty() {
            self.system
                .refresh_processes(ProcessesToUpdate::Some(&pids), true);
        }

        for process in &mut self.processes {
            let sys_proc = process
                .process_id
                .and_then(|pid| self.system.process(Pid::from(pid as usize)));

            if let Some(sys_proc) = sys_proc {
                process.mem = sys_proc.memory() / 1024 / 1024;
                process.cpu = sys_proc.cpu_usage() / self.system.cpus().len() as f32;
            } else {
                process.process_id = Some(0);
                process.mem = 0;
                process.cpu = 0.0;
            }
        }
    }

    async fn shutdown_with_modal(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let total = self.processes.len();
        self.shutdown_states = Some(vec![ShutdownStatus::Pending; total]);

        for idx in 0..total {
            if let Some(states) = &mut self.shutdown_states {
                states[idx] = ShutdownStatus::Stopping;
            }
            terminal.draw(|frame| frame.render_widget(&*self, frame.area()))?;
            sleep(Duration::from_millis(120)).await;

            if let Some(process) = self.processes.get_mut(idx) {
                let _ = process.kill().await;
            }

            if let Some(states) = &mut self.shutdown_states {
                states[idx] = ShutdownStatus::Done;
            }
            terminal.draw(|frame| frame.render_widget(&*self, frame.area()))?;
            sleep(Duration::from_millis(120)).await;
        }

        self.shutdown_states = None;
        self.running = false;
        Ok(())
    }

    pub async fn quit(&mut self) -> Result<()> {
        for process in &mut self.processes {
            process.kill().await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        self.running = false;
        Ok(())
    }
}
