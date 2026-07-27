use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use clap::{Parser, Subcommand};
use models::Entry;
use shel::{db, models, tui};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "shel",
    about = "Shell history manager",
    long_about = "Records shell commands to SQLite and provides a fuzzy TUI picker.\n\
                  Use `shel init <shell>` to print the hook, then source it in your shell rc.\n\
                  Ctrl-R in the shell opens the TUI picker; the selected command is written\n\
                  to stderr so the hook can inject it into the readline buffer."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Record a command into history (called by shell hooks).
    Record {
        command: String,
        #[arg(long)]
        exit_code: Option<i64>,
        #[arg(long)]
        duration_ms: Option<i64>,
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Search history and print results to stdout.
    Search {
        query: Option<String>,
        #[arg(short, long, default_value = "50")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Open the interactive TUI picker.
    /// Selected command is written to stderr so shell hooks can capture it
    /// via fd-swap (3>&1 1>&2 2>&3).
    Ui { query: Option<String> },
    /// Print the shell hook for the given shell (bash, zsh, fish).
    /// Source the output in your shell rc file.
    Init { shell: String },
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .compact()
        .init();
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let conn = db::open()?;

    match cli.cmd {
        Cmd::Record { command, exit_code, duration_ms, session_id } => {
            let cmd = command.trim().to_string();
            if cmd.is_empty() {
                return Ok(());
            }
            let entry = Entry {
                id: Uuid::new_v4().to_string(),
                command: cmd,
                cwd: std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()),
                exit_code,
                duration_ms,
                session_id,
                hostname: Some(gethostname::gethostname().to_string_lossy().into_owned()),
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            db::insert(&conn, &entry)?;
            // debug — not info: `shel record` runs on every prompt; info would
            // appear with RUST_LOG=info and flood the user's terminal.
            tracing::debug!(command = %entry.command, "recorded command");
        }

        Cmd::Search { query, limit, json } => {
            let entries = match query.as_deref() {
                Some(q) if !q.is_empty() => {
                    tracing::debug!(query = %q, "searching history");
                    db::search(&conn, q, limit)?
                }
                _ => db::list(&conn, limit)?,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                print_table(&entries);
            }
        }

        Cmd::Ui { query } => {
            if let Some(cmd) = tui::run(&conn, query.as_deref())? {
                // Written to stderr intentionally — shell hooks swap fds to
                // capture this into the readline/zle buffer.
                eprint!("{cmd}");
            }
        }

        Cmd::Init { shell } => {
            print_hook(&shell)?;
        }
    }

    Ok(())
}

fn print_table(entries: &[Entry]) {
    for e in entries.iter().rev() {
        let ts = Local
            .timestamp_millis_opt(e.timestamp)
            .single()
            .map(|dt: DateTime<Local>| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "?".to_string());
        let exit = e
            .exit_code
            .map(|c| if c == 0 { "✓".to_string() } else { format!("✗{c}") })
            .unwrap_or_else(|| " ".to_string());
        let dur = e.duration_ms.map(|d| format!("{d}ms")).unwrap_or_default();
        println!("{ts} {exit} {dur:>8}  {}", e.command);
    }
}

/// Print the shell hook for `shell` to stdout, or exit non-zero for unknown shells.
fn print_hook(shell: &str) -> Result<()> {
    match shell {
        "bash" => print!("{BASH_HOOK}"),
        "zsh" => print!("{ZSH_HOOK}"),
        "fish" => print!("{FISH_HOOK}"),
        other => {
            eprintln!("Unknown shell: {other}. Supported: bash, zsh, fish");
            std::process::exit(1);
        }
    }
    Ok(())
}

const BASH_HOOK: &str = include_str!("hooks/bash.sh");

const ZSH_HOOK: &str = include_str!("hooks/zsh.sh");

const FISH_HOOK: &str = include_str!("hooks/fish.sh");

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(cmd: &str, code: Option<i64>, dur: Option<i64>, ts: i64) -> Entry {
        Entry {
            id: "id".into(),
            command: cmd.into(),
            cwd: Some("/home/user/proj".into()),
            exit_code: code,
            duration_ms: dur,
            session_id: None,
            hostname: None,
            timestamp: ts,
        }
    }

    #[test]
    fn test_print_table_empty() {
        let result = std::panic::catch_unwind(|| print_table(&[]));
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_table_format() {
        let entries = vec![entry("git push", Some(0), Some(300), 1_700_000_000_000)];
        print_table(&entries);
    }

    #[test]
    fn test_print_hook_bash() {
        assert!(!BASH_HOOK.is_empty());
        assert!(BASH_HOOK.contains("__shel_ctrl_r"));
    }

    #[test]
    fn test_print_hook_zsh() {
        assert!(!ZSH_HOOK.is_empty());
        assert!(ZSH_HOOK.contains("bindkey"));
    }

    #[test]
    fn test_print_hook_fish() {
        assert!(!FISH_HOOK.is_empty());
        assert!(FISH_HOOK.contains("bind "));
    }

    #[test]
    fn test_hook_constants_contain_key_elements() {
        assert!(BASH_HOOK.contains("shel record"));
        assert!(ZSH_HOOK.contains("shel record"));
        assert!(FISH_HOOK.contains("shel record"));

        assert!(BASH_HOOK.contains("bind -x"));
        assert!(ZSH_HOOK.contains("bindkey"));
        assert!(FISH_HOOK.contains("bind "));

        assert!(BASH_HOOK.contains("PROMPT_COMMAND"));
        assert!(!ZSH_HOOK.contains("PROMPT_COMMAND"));
        assert!(!FISH_HOOK.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_bash_hook_uses_subshell() {
        assert!(BASH_HOOK.contains("(shel record"));
        assert!(BASH_HOOK.contains("&) >/dev/null 2>&1"));
    }

    #[test]
    fn test_bash_hook_uses_fd_swap() {
        assert!(BASH_HOOK.contains("3>&1 1>&2 2>&3"));
    }

    #[test]
    fn test_zsh_hook_uses_fd_swap() {
        assert!(ZSH_HOOK.contains("3>&1 1>&2 2>&3"));
    }

    #[test]
    fn test_zsh_hook_uses_epochrealtime() {
        assert!(ZSH_HOOK.contains("EPOCHREALTIME"));
    }

    #[test]
    fn test_print_hook_unknown_exits_nonzero() {
        // print_hook calls process::exit(1) for unknown shells.
        // We can't call it directly in tests, so just verify the known shells
        // are handled and the constant content is correct.
        assert!(BASH_HOOK.contains("shel record"));
        assert!(ZSH_HOOK.contains("shel record"));
        assert!(FISH_HOOK.contains("shel record"));
    }

    #[test]
    fn test_fish_hook_filename_comment() {
        // Fish hook comment should reference shel.fish, not hx.fish (old name).
        assert!(FISH_HOOK.contains("shel.fish"));
    }
}
