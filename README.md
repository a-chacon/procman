# proctui

A terminal user interface (TUI) for managing Procfile-based applications. `proctui` allows you to monitor, search, and interact with multiple processes simultaneously in a single dashboard.

## Captures

### Grid

![proctui grid mode](docs/images/grid.png)

### Fullscreen

![proctui fullscreen process](docs/images/fullscreen.png)

### Interactive

![proctui interactive mode](docs/images/interactive.png)

## Features

- **Procfile Support**: Automatically loads and manages processes defined in your `Procfile`.
- **Interactive Terminal**: Full VT100 terminal emulation for interacting with processes (perfect for debuggers like `gdb` or `lldb`).
- **Log Management**:
  - Real-time log streaming with ANSI color support.
  - Search through logs with highlighting.
  - Filter logs to focus on specific output.
- **Process Control**: Start, stop, and restart individual processes with single keybindings.
- **Flexible Layout**: Toggle between a grid view of all processes and a fullscreen view for deep dives.

## Installation

For now, you can install `proctui` via [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):

```bash
cargo install proctui
```

(Releases and pre-compiled executables coming soon.)

## Quick Start

1. Ensure you have a `Procfile` in your project root.
2. Run `proctui`:

   ```bash
   proctui
   ```

   Or specify a custom path to your `Procfile`:

   ```bash
   proctui ./path/to/my/Procfile
   ```

## Usage

`proctui` is designed to be intuitive and fast. Inspired by tools like `btop`, you can perform actions by simply pressing the **bolded** letter displayed in each label on the screen.

- **i**nteractive: Enter interactive mode to talk to the process (e.g., a debugger). Press `Ctrl-A` to exit.
- **f**ullscreen / **Enter**: Expand the selected process to fill the screen.
- se**a**rch: Find and highlight specific text in the logs.
- filte**r**: Hide lines that don't match your criteria.
- star**t**: Start a stopped process.
- **s**top: Terminate a running process.
- r**e**start: Quickly stop and start the process.
- **q** / **Ctrl-C**: Exit `proctui`.

Navigation is handled via **arrow keys** or **hjkl**. You can also jump directly to a process by pressing its corresponding number **1-9**.

## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).

By using this software, you agree to the terms outlined in the license.
