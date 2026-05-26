# shel

> Shell history that actually remembers. A `Ctrl-R` replacement backed by SQLite.

`shel` records every command you run — exit code, duration, working directory, hostname — and gives you a fast fuzzy TUI to search and re-run them. Drop it into bash, zsh, or fish in 30 seconds.

---

## Install

### Linux packages

Download `.deb` or `.rpm` from the [latest release](https://github.com/Lordsofzzzz/shel/releases/latest).

```bash
# Debian / Ubuntu
sudo dpkg -i shel_*.deb

# Fedora / RHEL / openSUSE
sudo rpm -i shel_*.rpm
```

Packages are available for **x86_64** and **aarch64**.

### From source

```bash
cargo install --git https://github.com/Lordsofzzzz/shel
```

Requires Rust 1.81+.

---

## Setup

Run once, add to your shell rc:

```bash
# Bash  (~/.bashrc)
eval "$(shel init bash)"

# Zsh   (~/.zshrc)
eval "$(shel init zsh)"

# Fish  (~/.config/fish/config.fish)
shel init fish | source
```

That's it. `Ctrl-R` now opens the TUI. Every command you run is recorded automatically in the background.

---

## TUI

```
  42 results
  > git

▶ ✓   1.2s  git push origin main
  ✓    43ms  git commit -m "fix: clippy warning"
  ✓    12ms  git log --oneline -10
  ✗1   8.4s  git rebase -i HEAD~5
```

| Key | Action |
|---|---|
| Type | Filter results |
| `↑` / `↓` or `Ctrl-P` / `Ctrl-N` | Navigate |
| `Tab` / `Shift-Tab` | Next / previous |
| `Enter` | Select and inject into prompt |
| `Ctrl-W` | Delete last word |
| `Ctrl-U` | Clear query |
| `Esc` / `Ctrl-C` | Cancel |

---

## CLI

```bash
# Search and print to stdout
shel search "git push"
shel search "cargo" --limit 20
shel search "docker" --json

# Open TUI with a pre-filled query
shel ui "kubectl"

# Record a command manually
shel record "make build" --exit-code 0 --duration-ms 4200

# Print the shell hook (for manual inspection)
shel init bash
```

---

## How it works

Shell hooks use `PROMPT_COMMAND` / `preexec` to capture each command *after* it finishes — so exit code and duration are always available. `shel record` runs in a background subshell so it never slows down your prompt.

History is stored in `~/.local/share/shel/history.db` (SQLite, WAL mode). Multiple terminal sessions write concurrently without lock contention.

The TUI uses [ratatui](https://ratatui.rs) with an **inline viewport** — it renders a small overlay below the current prompt line, not a full-screen takeover.

---

## Features

- Fuzzy search across full command history
- Exit code and duration visible at a glance (`✓` / `✗`)
- Per-command metadata: cwd, hostname, session ID, timestamp
- Works across multiple concurrent terminal sessions
- JSON output for scripting (`--json`)
- No daemon, no background process, no config file required

---

## License

MIT — see [LICENSE](LICENSE).
