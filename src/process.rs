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
use bytes::Bytes;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::style::Color;
use std::io::{Read, Write};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc::{Sender, channel};
use vt100::Parser;

pub struct Process {
    pub name: String,
    pub command: String,
    pub color: Color,
    pub parser: Arc<RwLock<Parser>>,
    pub sender: Option<Sender<Bytes>>,
    pub master_pty: Option<Box<dyn MasterPty + Send>>,
    pub process_id: Option<u32>,
    pub child_killer: Option<Box<dyn ChildKiller + Send + Sync>>,
    pub status: ProcessStatus,
    pub exited: Arc<AtomicBool>,
    pub scrollback: usize,
    pub mem: u64,
    pub cpu: f32,
    pub start_time: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Stopped,
}

impl Process {
    const SCROLLBACK_CAPACITY: usize = 2000;

    pub fn new(name: String, command: String, color: Color) -> Self {
        Self {
            name,
            command,
            color,
            parser: Arc::new(RwLock::new(Parser::new(24, 80, Self::SCROLLBACK_CAPACITY))),
            sender: None,
            master_pty: None,
            process_id: None,
            child_killer: None,
            status: ProcessStatus::Stopped,
            exited: Arc::new(AtomicBool::new(false)),
            scrollback: 0,
            mem: 0,
            cpu: 0.0,
            start_time: 0,
        }
    }

    pub async fn spawn(&mut self) -> Result<()> {
        self.spawn_with_size(24, 80).await
    }

    pub async fn spawn_with_size(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = rows.max(1);
        let cols = cols.max(1);

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
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
            // Use `exec` so the shell process is replaced by the target command.
            // This keeps the same PID, allowing direct per-PID stats lookups later.
            let mut c = CommandBuilder::new("sh");
            c.arg("-c");
            c.arg(format!("exec {}", self.command));
            c.env("TERM", "xterm-256color");
            c
        };

        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        self.status = ProcessStatus::Running;
        self.exited.store(false, Ordering::Relaxed);
        self.scrollback = 0;

        {
            let mut parser = self.parser.write().unwrap();
            parser.screen_mut().set_size(rows, cols);
            parser.screen_mut().set_scrollback(0);
        }

        // Spawn child process and keep a killer handle for graceful shutdown.
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;
        self.process_id = child.process_id();
        self.child_killer = Some(child.clone_killer());

        let exited_clone = self.exited.clone();
        std::thread::spawn(move || {
            let _ = child.wait();
            exited_clone.store(true, Ordering::Relaxed);
            drop(pair.slave);
        });

        // Setup PTY reader on a dedicated thread (blocking IO).
        let mut reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let parser = self.parser.clone();
        std::thread::spawn(move || {
            let mut processed_buf = Vec::new();
            let mut buf = [0u8; 8192];

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(size) => {
                        processed_buf.extend_from_slice(&buf[..size]);
                        let mut parser = parser.write().unwrap();
                        parser.process(&processed_buf);
                        processed_buf.clear();
                    }
                    Err(_) => break,
                }
            }
        });

        // Setup PTY writer on a dedicated thread (blocking IO).
        let (tx, mut rx) = channel::<Bytes>(32);
        let mut writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        std::thread::spawn(move || {
            while let Some(bytes) = rx.blocking_recv() {
                let _ = writer.write_all(&bytes);
                let _ = writer.flush();
            }
        });

        self.sender = Some(tx);
        self.master_pty = Some(pair.master);

        Ok(())
    }

    pub async fn kill(&mut self) -> Result<()> {
        if let Some(killer) = &mut self.child_killer {
            let _ = killer.kill();
        }

        self.master_pty = None;
        self.sender = None;
        self.child_killer = None;
        self.status = ProcessStatus::Stopped;
        self.exited.store(true, Ordering::Relaxed);
        self.scrollback = 0;

        // Give it a moment to clean up
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    pub async fn write_input(&mut self, input: Bytes) -> Result<()> {
        if let Some(sender) = &self.sender {
            sender
                .send(input)
                .await
                .context("Failed to send input to PTY")?;
        }
        Ok(())
    }

    pub fn resize_pty(&mut self, rows: u16, cols: u16) -> Result<()> {
        if let Some(master) = &self.master_pty {
            master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .context("Failed to resize PTY")?;

            let mut parser = self.parser.write().unwrap();
            parser.screen_mut().set_size(rows, cols);
            parser.screen_mut().set_scrollback(self.scrollback);
        }
        Ok(())
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scrollback = self.scrollback.saturating_add(lines);
        let mut parser = self.parser.write().unwrap();
        parser.screen_mut().set_scrollback(self.scrollback);
        self.scrollback = parser.screen().scrollback();
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scrollback = self.scrollback.saturating_sub(lines);
        let mut parser = self.parser.write().unwrap();
        parser.screen_mut().set_scrollback(self.scrollback);
        self.scrollback = parser.screen().scrollback();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scrollback = 0;
        let mut parser = self.parser.write().unwrap();
        parser.screen_mut().set_scrollback(0);
    }

    pub fn is_alive(&self) -> bool {
        !self.exited.load(Ordering::Relaxed)
    }
}
