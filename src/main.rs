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

pub mod app;
pub mod event;
pub mod formation;
pub mod process;
pub mod procfile;
pub mod ui;
use clap::Parser;
use formation::Formation;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Specify an alternate location for the application's Procfile.
    #[arg(short, long)]
    procfile: Option<PathBuf>,

    /// Specify the number of each process type to run. The value passed in should be in the format process=num,process=num
    #[arg(short = 'm', long)]
    formation: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let formation = args.formation.map(Formation::from);

    let procfile_path = if let Some(procfile) = args.procfile {
        String::from(procfile.to_str().unwrap())
    } else {
        String::from("Procfile")
    };

    let terminal = ratatui::init();
    let result = app::App::new(procfile_path, formation).run(terminal).await;
    ratatui::restore();
    result
}
