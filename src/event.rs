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

use anyhow::Result;
use crossterm::event::Event as CrosstermEvent;
use futures::{FutureExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;

/// The frequency at which tick events are emitted.
const TICK_FPS: f64 = 30.0;

/// Representation of all possible events.
#[derive(Clone, Debug)]
pub enum Event {
    /// An event that is emitted on a regular schedule.
    Tick,
    /// Crossterm events.
    Crossterm(CrosstermEvent),
    /// Application events.
    App(AppEvent),
}

/// Application events.
#[derive(Clone, Debug)]
pub enum AppEvent {
    /// Quit the application.
    Quit,
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl Default for EventHandler {
    fn default() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await });

        Self { sender, receiver }
    }
}

impl EventHandler {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn next(&mut self) -> Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to receive event"))
    }

    pub fn send(&mut self, app_event: AppEvent) {
        let _ = self.sender.send(Event::App(app_event));
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
struct EventTask {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    /// Constructs a new instance of [`EventTask`].
    fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    /// Runs the event thread.
    async fn run(self) -> Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              _ = tick_delay => {
                self.send(Event::Tick);
              }
              Some(Ok(evt)) = crossterm_event => {
                self.send(Event::Crossterm(evt));
              }
            };
        }
        Ok(())
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}
