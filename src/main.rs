use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use clap::{Parser, Subcommand};
use hx::{db, models, tui};
use models::Entry;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "hx", about = "Shell history manager")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Record {
        command: String,
        #[arg(long)] exit_code:   Option<i64>,
        #[arg(long)] duration_ms: Option<i64>,
        #[arg(long)] session_id:  Option<String>,
    },
    Search {
        query: Option<String>,
        #[arg(short, long, default_value = "50")] limit: usize,
        #[arg(long)] json: bool,
    },
    Ui {
        query: Option<String>,
    },
    Init {
        shell: String,
    },
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
            if cmd.is_empty() { return Ok(()); }
            let entry = Entry {
                id:          Uuid::new_v4().to_string(),
                command:     cmd,
                cwd:         std::env::current_dir().ok()
                                 .map(|p| p.to_string_lossy().to_string()),
                exit_code,
                duration_ms,
                session_id,
                hostname:    hostname::get().ok()
                                 .map(|h| h.to_string_lossy().to_string()),
                timestamp:   chrono::Utc::now().timestamp_millis(),
            };
            db::insert(&conn, &entry)?;
            tracing::info!(command = %entry.command, "recorded command");
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
                eprint!("{}", cmd);
            }
        }

        Cmd::Init { shell } => {
            print_hook(&shell);
        }
    }

    Ok(())
}

fn print_table(entries: &[Entry]) {
    for e in entries.iter().rev() {
        let ts = Local.timestamp_millis_opt(e.timestamp)
            .single()
            .map(|dt: DateTime<Local>| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "?".to_string());
        let exit = e.exit_code
            .map(|c| if c == 0 { "✓".to_string() } else { format!("✗{}", c) })
            .unwrap_or_else(|| " ".to_string());
        let dur = e.duration_ms.map(|d| format!("{}ms", d)).unwrap_or_default();
        println!("{} {} {:>8}  {}", ts, exit, dur, e.command);
    }
}

fn print_hook(shell: &str) {
    match shell {
        "bash" => print!("{}", BASH_HOOK),
        "zsh"  => print!("{}", ZSH_HOOK),
        "fish" => print!("{}", FISH_HOOK),
        _ => eprintln!("Unknown shell: {}. Supported: bash, zsh, fish", shell),
    }
}

const BASH_HOOK: &str = r#"
# hx bash hook — add to ~/.bashrc
__hx_session_id=$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || date +%s)
__hx_start_time=

__hx_preexec() {
    __hx_start_time=$(date +%s%3N)
}

__hx_precmd() {
    local exit_code=$?
    local cmd
    cmd=$(HISTTIMEFORMAT= history 1 | sed 's/^ *[0-9]* *//')
    [[ -z "$cmd" ]] && return
    local duration_ms=0
    if [[ -n "$__hx_start_time" ]]; then
        duration_ms=$(( $(date +%s%3N) - __hx_start_time ))
    fi
    (hx record "$cmd" \
        --exit-code "$exit_code" \
        --duration-ms "$duration_ms" \
        --session-id "$__hx_session_id" &) >/dev/null 2>&1
    __hx_start_time=
}

__hx_ctrl_r() {
    local selected
    selected=$(hx ui "$READLINE_LINE" 3>&1 1>&2 2>&3)
    if [[ -n "$selected" ]]; then
        READLINE_LINE="$selected"
        READLINE_POINT=${#READLINE_LINE}
    fi
}

trap '__hx_preexec' DEBUG
PROMPT_COMMAND="__hx_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
bind -x '"\C-r": __hx_ctrl_r'
"#;

const ZSH_HOOK: &str = r#"
# hx zsh hook — add to ~/.zshrc
__hx_session_id=$(uuidgen 2>/dev/null || date +%s)
__hx_start_time=

__hx_preexec() {
    __hx_start_time=${EPOCHREALTIME-$(date +%s%3N)}
}

__hx_precmd() {
    local exit_code=$?
    local cmd=$history[$HISTCMD]
    [[ -z "$cmd" ]] && return
    local duration_ms=0
    if [[ -n "$__hx_start_time" ]]; then
        duration_ms=$(( $(date +%s%3N) - ${__hx_start_time%%.*}000 ))
    fi
    (hx record "$cmd" \
        --exit-code "$exit_code" \
        --duration-ms "$duration_ms" \
        --session-id "$__hx_session_id" &) >/dev/null 2>&1
    __hx_start_time=
}

__hx_ctrl_r() {
    zle -I
    local selected
    selected=$(hx ui "$BUFFER" 3>&1 1>&2 2>&3)
    zle reset-prompt
    [[ -z "$selected" ]] && return
    BUFFER="$selected"
    CURSOR=${#BUFFER}
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec __hx_preexec
add-zsh-hook precmd __hx_precmd
zle -N __hx_ctrl_r
bindkey '^R' __hx_ctrl_r
"#;

const FISH_HOOK: &str = r#"
# hx fish hook — save to ~/.config/fish/conf.d/hx.fish
set -g __hx_session_id (uuidgen 2>/dev/null; or date +%s)
set -g __hx_start_time 0

function __hx_on_event --on-event fish_preexec
    set __hx_start_time (date +%s%3N)
end

function __hx_on_postexec --on-event fish_postexec
    set -l exit_code $status
    set -l cmd $argv[1]
    set -l duration_ms 0
    if test $__hx_start_time -gt 0
        set duration_ms (math (date +%s%3N) - $__hx_start_time)
    end
    hx record "$cmd" \
        --exit-code $exit_code \
        --duration-ms $duration_ms \
        --session-id $__hx_session_id &>/dev/null &
    set __hx_start_time 0
end

function __hx_ctrl_r
    set -l selected (hx ui (commandline) 2>/dev/null)
    if test -n "$selected"
        commandline -- $selected
    end
    commandline -f repaint
end

bind \cr __hx_ctrl_r
"#;

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
        let output = std::panic::catch_unwind(|| print_table(&[]));
        assert!(output.is_ok());
    }

    #[test]
    fn test_print_table_format() {
        let entries = vec![
            entry("git push", Some(0), Some(300), 1_700_000_000_000),
        ];
        print_table(&entries);
    }

    #[test]
    fn test_print_hook_bash() {
        assert!(!BASH_HOOK.is_empty());
        assert!(BASH_HOOK.contains("__hx_ctrl_r"));
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
    fn test_print_hook_unknown_shell() {
        print_hook("unknown");
    }

    #[test]
    fn test_hook_constants_contain_key_elements() {
        assert!(BASH_HOOK.contains("hx record"));
        assert!(ZSH_HOOK.contains("hx record"));
        assert!(FISH_HOOK.contains("hx record"));

        assert!(BASH_HOOK.contains("bind -x"));
        assert!(ZSH_HOOK.contains("bindkey"));
        assert!(FISH_HOOK.contains("bind "));

        assert!(BASH_HOOK.contains("PROMPT_COMMAND"));
        assert!(!ZSH_HOOK.contains("PROMPT_COMMAND"));
        assert!(!FISH_HOOK.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_bash_hook_uses_subshell() {
        assert!(BASH_HOOK.contains("(hx record"));
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
}
