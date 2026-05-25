use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Terminal, TerminalOptions, Viewport,
};
use rusqlite::Connection;
use std::io::{self, Write};

use crate::db;
use crate::models::Entry;

const POPUP_HEIGHT: u16 = 14;

struct App {
    query:      String,
    entries:    Vec<Entry>,
    filtered:   Vec<usize>,
    list_state: ListState,
    selected:   Option<String>,
    done:       bool,
}

impl App {
    fn new(entries: Vec<Entry>, initial_query: &str) -> Self {
        let mut app = App {
            query: initial_query.to_string(),
            entries,
            filtered: vec![],
            list_state: ListState::default(),
            selected: None,
            done: false,
        };
        app.filter();
        app
    }

    fn filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self.entries.iter().enumerate()
            .filter(|(_, e)| e.command.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        self.list_state.select(if self.filtered.is_empty() { None } else { Some(0) });
    }

    fn up(&mut self) {
        let n = self.filtered.len();
        if n == 0 { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if i == 0 { n - 1 } else { i - 1 }));
    }

    fn down(&mut self) {
        let n = self.filtered.len();
        if n == 0 { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % n));
    }

    fn confirm(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(&ei) = self.filtered.get(idx) {
                self.selected = Some(self.entries[ei].command.clone());
            }
        }
        self.done = true;
    }
}

/// Run the interactive TUI overlay. Returns the selected command if the user
/// confirmed a choice, or `None` if they cancelled.
pub fn run(conn: &Connection, initial_query: Option<&str>) -> Result<Option<String>> {
    let entries = db::list(conn, 10000)?;
    let mut app = App::new(entries, initial_query.unwrap_or(""));

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    for _ in 0..POPUP_HEIGHT {
        writeln!(stdout)?;
    }
    execute!(stdout, cursor::MoveUp(POPUP_HEIGHT))?;
    stdout.flush()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions { viewport: Viewport::Inline(POPUP_HEIGHT) },
    )?;

    loop {
        terminal.draw(|f| render(f, &mut app))?;
        if app.done { break; }

        if let Event::Key(key) = event::read()? {
            match (key.modifiers, key.code) {
                (_, KeyCode::Esc)
                | (KeyModifiers::CONTROL, KeyCode::Char('c'))
                | (KeyModifiers::CONTROL, KeyCode::Char('g')) => { app.done = true; break; }

                (_, KeyCode::Enter)  => app.confirm(),

                (_, KeyCode::Up)
                | (KeyModifiers::CONTROL, KeyCode::Char('p')) => app.up(),
                (_, KeyCode::Down)
                | (KeyModifiers::CONTROL, KeyCode::Char('n'))
                | (_, KeyCode::Tab)  => app.down(),
                (_, KeyCode::BackTab) => app.up(),

                (_, KeyCode::Backspace) => { app.query.pop(); app.filter(); }
                (KeyModifiers::CONTROL, KeyCode::Char('u')) => { app.query.clear(); app.filter(); }
                (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                    let t = app.query.trim_end();
                    let end = t.rfind(' ').map(|i| i + 1).unwrap_or(0);
                    app.query.truncate(end);
                    app.filter();
                }
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                    app.query.push(c);
                    app.filter();
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    terminal.clear()?;
    while crossterm::event::poll(std::time::Duration::from_millis(10))? {
        let _ = crossterm::event::read();
    }
    Ok(app.selected)
}

fn fmt_dur(ms: i64) -> String {
    if ms < 1000        { format!("{ms}ms") }
    else if ms < 60_000 { format!("{:.1}s", ms as f64 / 1000.0) }
    else                { format!("{:.0}m", ms as f64 / 60_000.0) }
}

fn render(f: &mut ratatui::Frame, app: &mut App) {
    let area = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    let count = app.filtered.len();
    let search = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{count} results"), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  > ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(app.query.clone()),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ]),
    ]);
    f.render_widget(search, chunks[0]);

    let selected_idx = app.list_state.selected();

    let dim      = Style::default().fg(Color::DarkGray);
    let ok       = Style::default().fg(Color::Green);
    let err      = Style::default().fg(Color::Red);
    let cmd_norm = Style::default().fg(Color::White);
    let cmd_sel  = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);

    let items: Vec<ListItem> = app.filtered.iter().enumerate().map(|(list_idx, &ei)| {
        let e = &app.entries[ei];
        let is_sel = selected_idx == Some(list_idx);

        let arrow = if is_sel {
            Span::styled("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("  ")
        };

        let (mark, m_style) = match e.exit_code {
            Some(0) => ("✓", ok),
            Some(_) => ("✗", err),
            None    => ("·", dim),
        };

        let dur = e.duration_ms
            .map(|d| format!("{:>6}  ", fmt_dur(d)))
            .unwrap_or_else(|| "        ".to_string());

        let cmd_style = if is_sel { cmd_sel } else { cmd_norm };

        ListItem::new(Line::from(vec![
            arrow,
            Span::styled(format!("{mark} "), m_style),
            Span::styled(dur, dim),
            Span::styled(e.command.clone(), cmd_style),
        ]))
    }).collect();

    let list = List::new(items);
    let mut state = app.list_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, cmd: &str, ts: i64) -> Entry {
        Entry {
            id: id.into(),
            command: cmd.into(),
            cwd: None,
            exit_code: Some(0),
            duration_ms: None,
            session_id: None,
            hostname: None,
            timestamp: ts,
        }
    }

    #[test]
    fn test_app_new_empty() {
        let app = App::new(vec![], "");
        assert!(app.query.is_empty());
        assert!(app.filtered.is_empty());
        assert!(app.list_state.selected().is_none());
        assert!(!app.done);
    }

    #[test]
    fn test_app_new_with_initial_query() {
        let entries = vec![make_entry("a", "git push", 1)];
        let app = App::new(entries, "git");
        assert_eq!(app.query, "git");
    }

    #[test]
    fn test_filter_basic() {
        let entries = vec![
            make_entry("a", "git push",   1),
            make_entry("b", "cargo build", 2),
            make_entry("c", "git log",    3),
        ];
        let mut app = App::new(entries, "");
        app.query = "git".into();
        app.filter();
        assert_eq!(app.filtered.len(), 2);
    }

    #[test]
    fn test_filter_case_insensitive() {
        let entries = vec![
            make_entry("a", "GIT PUSH", 1),
            make_entry("b", "npm run",  2),
        ];
        let mut app = App::new(entries, "");
        app.query = "git".into();
        app.filter();
        assert_eq!(app.filtered.len(), 1);
    }

    #[test]
    fn test_filter_no_match_clears_selection() {
        let entries = vec![make_entry("a", "git push", 1)];
        let mut app = App::new(entries, "");
        app.query = "zzz".into();
        app.filter();
        assert!(app.filtered.is_empty());
        assert!(app.list_state.selected().is_none());
    }

    #[test]
    fn test_filter_empty_query_shows_all() {
        let entries = vec![
            make_entry("a", "git push",   1),
            make_entry("b", "cargo build", 2),
        ];
        let mut app = App::new(entries, "");
        app.query = "".into();
        app.filter();
        assert_eq!(app.filtered.len(), 2);
    }

    #[test]
    fn test_up_wraps_to_bottom() {
        let entries = vec![
            make_entry("a", "git push", 1),
            make_entry("b", "cargo",    2),
        ];
        let mut app = App::new(entries, "");
        assert_eq!(app.list_state.selected(), Some(0));
        app.up();
        assert_eq!(app.list_state.selected(), Some(1));
        app.up();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_down_wraps_to_top() {
        let entries = vec![
            make_entry("a", "git push", 1),
            make_entry("b", "cargo",    2),
        ];
        let mut app = App::new(entries, "");
        assert_eq!(app.list_state.selected(), Some(0));
        app.down();
        assert_eq!(app.list_state.selected(), Some(1));
        app.down();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_up_down_on_empty_filtered() {
        let mut app = App::new(vec![], "");
        app.up();
        app.down();
    }

    #[test]
    fn test_confirm_selects_command() {
        let entries = vec![
            make_entry("a", "git push", 1),
            make_entry("b", "cargo",    2),
        ];
        let mut app = App::new(entries, "");
        app.down();
        app.confirm();
        assert_eq!(app.selected.as_deref(), Some("cargo"));
        assert!(app.done);
    }

    #[test]
    fn test_confirm_with_no_selection() {
        let mut app = App::new(vec![], "");
        app.confirm();
        assert!(app.selected.is_none());
        assert!(app.done);
    }

    #[test]
    fn test_confirm_with_filtered_list() {
        let entries = vec![
            make_entry("a", "git push",   1),
            make_entry("b", "npm install", 2),
            make_entry("c", "git log",    3),
        ];
        let mut app = App::new(entries, "");
        app.query = "npm".into();
        app.filter();
        app.confirm();
        assert_eq!(app.selected.as_deref(), Some("npm install"));
    }

    #[test]
    fn test_fmt_dur_ms() {
        assert_eq!(fmt_dur(0), "0ms");
        assert_eq!(fmt_dur(42), "42ms");
        assert_eq!(fmt_dur(999), "999ms");
    }

    #[test]
    fn test_fmt_dur_seconds() {
        assert_eq!(fmt_dur(1000), "1.0s");
        assert_eq!(fmt_dur(1500), "1.5s");
        assert_eq!(fmt_dur(59_999), "60.0s");
    }

    #[test]
    fn test_fmt_dur_minutes() {
        assert_eq!(fmt_dur(60_000), "1m");
        assert_eq!(fmt_dur(120_000), "2m");
        assert_eq!(fmt_dur(3_600_000), "60m");
    }
}
