# shel

Shell history manager with a fuzzy-search TUI picker — a modern `Ctrl-R` replacement.

Records every command to SQLite with exit code, duration, working directory, and hostname. The interactive TUI lets you search, pick, and re-run commands instantly.

## Features

- **Drop-in Ctrl-R replacement** — hooks for bash, zsh, and fish
- **Fast incremental search** — type to filter, arrows to navigate, Enter to select
- **Rich metadata** — exit code, duration, cwd, hostname, timestamp per command
- **SQLite-backed** — WAL mode for concurrent writers, no lock contention
- **CLI interface** — `shel search`, `shel record`, `shel init` for scripting

## Installation

### Linux (deb/rpm)

Download the `.deb` or `.rpm` from the [latest release](https://github.com/Lordsofzzzz/shel/releases/latest).

```bash
# Debian/Ubuntu
sudo dpkg -i shel_*.deb

# Fedora/RHEL
sudo rpm -i shel_*.rpm
```

### From source

```bash
cargo install shel
```

Requires Rust 1.81+.

## Shell Setup

Run the `init` command for your shell and add the output to your shell rc file.

**Bash** (`~/.bashrc`):

```bash
eval "$(shel init bash)"
```

**Zsh** (`~/.zshrc`):

```zsh
eval "$(shel init zsh)"
```

**Fish** (`~/.config/fish/config.fish`):

```fish
shel init fish | source
```

After sourcing the hook, `Ctrl-R` opens the TUI picker and running commands is automatically recorded.

## Usage

```
Usage: shel <COMMAND>

Commands:
  record   Record a command into history (called by shell hooks)
  search   Search history and print results to stdout
  ui       Open the interactive TUI picker
  init     Print the shell hook for the given shell (bash, zsh, fish)
  help     Print help
```

### Examples

```bash
# Search history from the CLI
shel search "git push"
shel search "cargo" --json

# Search with a pre-filled query in the TUI
shel ui "docker"

# Record a command manually
shel record "curl example.com" --exit-code 0 --duration-ms 250
```

## TUI Keybindings

| Key | Action |
|---|---|
| `Ctrl-R`, `Enter` | Select and return command |
| `Esc`, `Ctrl-C`, `Ctrl-G` | Cancel |
| `↑` / `↓` | Navigate list |
| `Ctrl-P` / `Ctrl-N` | Navigate list (vim-style) |
| `Tab` / `Shift-Tab` | Next / previous |
| `Backspace` | Delete character |
| `Ctrl-U` | Clear query |
| `Ctrl-W` | Delete last word |

## How It Works

`shel` uses `PROMPT_COMMAND` / `preexec` hooks to record each command **after** it finishes (so exit code and duration are known). The `record` subcommand runs in a background subshell to avoid slowing down your prompt.

The TUI uses [ratatui](https://ratatui.rs) with an inline viewport overlay — it doesn't take over the full terminal.

## License

MIT
