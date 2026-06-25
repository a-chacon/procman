# Procman

A terminal user interface (TUI) for managing Procfile-based applications. `procman` allows you to monitor, search, and interact with multiple processes simultaneously in a single dashboard.

## Captures

### Grid

![procman grid mode](docs/images/grid.png)

### Fullscreen

![procman fullscreen process](docs/images/fullscreen.png)

### Interactive

![procman interactive mode](docs/images/interactive.png)

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

For now, you can install `procman` via [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):

```bash
cargo install proc-man
```

Or with the official script (Linux and MacOS for the momment):

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/a-chacon/procman/releases/latest/download/proc-man-installer.sh | sh
```

## Quick Start

1. Ensure you have a `Procfile` in your project root.
2. Run `procman`:

   ```bash
   procman
   ```

   Or specify a custom path to your `Procfile`:

   ```bash
   procman ./path/to/my/Procfile

   ```

## For Rails Apps

To use `procman` as a drop-in replacement for Foreman in your Rails application, replace the contents of your `bin/dev` script with the following. This script automatically checks if `procman` is installed and installs it via the official installer if missing:

```bash
#!/usr/bin/env sh

# Check if procman is installed; if not, install it automatically
if ! command -v procman >/dev/null 2>&1; then
  echo "procman not found. Installing..."
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/a-chacon/procman/releases/latest/download/proc-man-installer.sh | sh
  
  # Refresh shell path if needed (depending on shell and install location)
fi

# Default to port 3000 if not specified
export PORT="${PORT:-3000}"

# Start processes via procman
exec procman Procfile.dev "$@"
```

## Usage

`procman` is designed to be intuitive and fast. Inspired by tools like `btop`, you can perform actions by simply pressing the **bolded** letter displayed in each label on the screen.

- **i**nteractive: Enter interactive mode to talk to the process (e.g., a debugger). Press `Ctrl-A` to exit.
- **f**ullscreen / **Enter**: Expand the selected process to fill the screen.
- se**a**rch: Find and highlight specific text in the logs.
- filte**r**: Hide lines that don't match your criteria.
- star**t**: Start a stopped process.
- **s**top: Terminate a running process.
- r**e**start: Quickly stop and start the process.
- **q** / **Ctrl-C**: Exit `procman`.

Navigation is handled via **arrow keys** or **hjkl**. You can also jump directly to a process by pressing its corresponding number **1-9**.

## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).

By using this software, you agree to the terms outlined in the license.
