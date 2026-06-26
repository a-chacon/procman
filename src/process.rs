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

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, PtyPair, PtySize, native_pty_system};
use ratatui::style::Color;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use vt100::Parser;

pub struct Process {
    pub name: String,
    pub command: String,
    pub color: Color,
    pub output: Arc<Mutex<VecDeque<String>>>,
    pub vt: Arc<Mutex<Parser>>,
    pub pty: Option<PtyPair>,
    pub writer: Option<Box<dyn Write + Send>>,
    pub child: Option<Box<dyn Child + Send + Sync>>,
    pub status: ProcessStatus,
    pub scroll: u16,
    pub filter: Option<String>,
    pub search_query: Option<String>,
    pub active_match_line: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Stopped,
    Failed,
}

impl Process {
    pub fn new(name: String, command: String, color: Color) -> Self {
        Self {
            name,
            command,
            color,
            output: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            vt: Arc::new(Mutex::new(Parser::new(24, 80, 1000))),
            pty: None,
            writer: None,
            child: None,
            status: ProcessStatus::Stopped,
            scroll: 0,
            filter: None,
            search_query: None,
            active_match_line: None,
        }
    }

    pub async fn spawn(&mut self) -> Result<()> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY")?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = CommandBuilder::new("cmd");
            c.arg("/C");
            c.arg(&self.command);
            c.env("TERM", "xterm-256color");
            c
        } else {
            let mut c = CommandBuilder::new("sh");
            c.arg("-c");
            c.arg(&self.command);
            c.env("TERM", "xterm-256color");
            c
        };

        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context(format!("Failed to spawn process {}", self.name))?;

        self.child = Some(child);
        self.status = ProcessStatus::Running;

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        self.writer = Some(writer);
        self.pty = Some(pair);

        let output_clone = self.output.clone();
        let vt_clone = self.vt.clone();
        tokio::task::spawn_blocking(move || {
            let mut buffer = [0u8; 4096];
            let mut last_char_was_cr = false;
            let mut in_esc = false;
            let mut esc_buffer = String::new();

            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                let data = &buffer[..n];

                // Feed to VT100 parser
                {
                    let mut vt = vt_clone.lock().unwrap();
                    vt.process(data);
                }

                let s = String::from_utf8_lossy(data);
                let mut lines = output_clone.lock().unwrap();
                if lines.is_empty() {
                    lines.push_back(String::new());
                }

                for c in s.chars() {
                    if in_esc {
                        esc_buffer.push(c);
                        // Basic support for "Clear Line" sequences
                        if c == 'K' {
                            if (esc_buffer.contains("[K") || esc_buffer.contains("[2K"))
                                && let Some(line) = lines.back_mut()
                            {
                                line.clear();
                            }
                            in_esc = false;
                            esc_buffer.clear();
                        } else if c.is_alphabetic() || esc_buffer.len() > 10 {
                            // End of escape sequence or too long
                            in_esc = false;
                            esc_buffer.clear();
                        }
                        continue;
                    }

                    match c {
                        '\x1b' => {
                            in_esc = true;
                            esc_buffer.clear();
                            esc_buffer.push(c);
                        }
                        '\n' => {
                            lines.push_back(String::new());
                            if lines.len() > 1000 {
                                lines.pop_front();
                            }
                            last_char_was_cr = false;
                        }
                        '\r' => {
                            last_char_was_cr = true;
                        }
                        '\x08' => {
                            if let Some(line) = lines.back_mut() {
                                line.pop();
                            }
                            last_char_was_cr = false;
                        }
                        c => {
                            if let Some(line) = lines.back_mut() {
                                if last_char_was_cr {
                                    line.clear();
                                }
                                line.push(c);
                            }
                            last_char_was_cr = false;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn kill(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            if let Some(pid) = child.process_id() {
                #[cfg(unix)]
                {
                    // Send SIGTERM to the process group.
                    // Note: portable-pty doesn't automatically create a new group
                    // unless we use setsid/setpgid, which we'll add to spawn.
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(-(pid as i32)),
                        nix::sys::signal::Signal::SIGTERM,
                    );
                }

                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .arg("/F")
                        .arg("/T")
                        .arg("/PID")
                        .arg(pid.to_string())
                        .output();
                }
            }

            let _ = child.kill();
            // We must wait to avoid zombies and ensure cleanup
            let _ = child.wait();
            self.status = ProcessStatus::Stopped;
        }
        self.pty = None;
        self.writer = None;
        Ok(())
    }

    pub fn write_input(&mut self, input: &[u8]) -> Result<()> {
        if let Some(writer) = &mut self.writer {
            writer.write_all(input).context("Failed to write to PTY")?;
            writer.flush().context("Failed to flush PTY writer")?;
        }
        Ok(())
    }

    pub fn resize_pty(&mut self, rows: u16, cols: u16) -> Result<()> {
        if let Some(pair) = &self.pty {
            pair.master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .context("Failed to resize PTY")?;

            let mut vt = self.vt.lock().unwrap();
            vt.set_size(rows, cols);
        }
        Ok(())
    }
}
