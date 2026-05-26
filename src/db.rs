use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use crate::models::Entry;

/// Open or create the SQLite history database, applying migrations.
///
/// Enables WAL mode and a 5-second busy timeout so concurrent writers
/// (multiple terminal sessions each spawning `shel record &`) don't
/// immediately return `SQLITE_BUSY`.
pub fn open() -> Result<Connection> {
    let path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("shel")
        .join("history.db");

    let parent = path.parent()
        .with_context(|| format!("invalid db path, no parent dir: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create db dir: {}", parent.display()))?;

    let conn = Connection::open(&path)
        .with_context(|| format!("failed to open db: {}", path.display()))?;

    // WAL: multiple readers + one writer without blocking each other.
    // synchronous=NORMAL: safe with WAL, faster than FULL.
    // busy_timeout: retry for 5 s instead of immediately failing.
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;",
    )?;

    migrate(&conn)?;
    Ok(conn)
}

/// Run database schema migrations (idempotent).
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id          TEXT PRIMARY KEY,
            command     TEXT NOT NULL,
            cwd         TEXT,
            exit_code   INTEGER,
            duration_ms INTEGER,
            session_id  TEXT,
            hostname    TEXT,
            timestamp   INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ts  ON history(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_cmd ON history(command);",
    )?;
    Ok(())
}

/// Insert an entry into the history table. Silently ignores duplicates.
pub fn insert(conn: &Connection, e: &Entry) -> Result<()> {
    // prepare_cached: statement is compiled once and reused on subsequent calls.
    let mut stmt = conn.prepare_cached(
        "INSERT OR IGNORE INTO history
         (id, command, cwd, exit_code, duration_ms, session_id, hostname, timestamp)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
    )?;
    stmt.execute(params![
        e.id, e.command, e.cwd, e.exit_code,
        e.duration_ms, e.session_id, e.hostname, e.timestamp,
    ])?;
    Ok(())
}

/// Search history for commands matching `query` (substring, case-insensitive
/// for ASCII), ordered by most recent first.
/// `%`, `_`, and `!` in the query are treated as literal characters.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Entry>> {
    let escaped = query
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_");
    let pattern = format!("%{escaped}%");
    let mut stmt = conn.prepare_cached(
        "SELECT id,command,cwd,exit_code,duration_ms,session_id,hostname,timestamp
         FROM history
         WHERE command LIKE ?1 ESCAPE '!'
         ORDER BY timestamp DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pattern, limit as i64], row_to_entry)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

/// List the most recent history entries, ordered by timestamp descending.
/// Pass `usize::MAX` (or use `list_all`) to retrieve everything.
pub fn list(conn: &Connection, limit: usize) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id,command,cwd,exit_code,duration_ms,session_id,hostname,timestamp
         FROM history ORDER BY timestamp DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], row_to_entry)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

/// List every history entry without a row cap. Used by the TUI.
pub fn list_all(conn: &Connection) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id,command,cwd,exit_code,duration_ms,session_id,hostname,timestamp
         FROM history ORDER BY timestamp DESC",
    )?;
    let rows = stmt.query_map([], row_to_entry)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id:          row.get(0)?,
        command:     row.get(1)?,
        cwd:         row.get(2)?,
        exit_code:   row.get(3)?,
        duration_ms: row.get(4)?,
        session_id:  row.get(5)?,
        hostname:    row.get(6)?,
        timestamp:   row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn
    }

    fn sample_entry(id: &str, cmd: &str, ts: i64) -> Entry {
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
    fn test_insert_and_list() {
        let conn = test_conn();
        let e = sample_entry("a1", "git push", 100);
        insert(&conn, &e).unwrap();

        let entries = list(&conn, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "git push");
        assert_eq!(entries[0].id, "a1");
    }

    #[test]
    fn test_list_ordered_by_timestamp_desc() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "first",  100)).unwrap();
        insert(&conn, &sample_entry("b", "second", 200)).unwrap();
        insert(&conn, &sample_entry("c", "third",  150)).unwrap();

        let entries = list(&conn, 10).unwrap();
        assert_eq!(entries[0].id, "b");
        assert_eq!(entries[1].id, "c");
        assert_eq!(entries[2].id, "a");
    }

    #[test]
    fn test_list_limit() {
        let conn = test_conn();
        for i in 0..5 {
            insert(&conn, &sample_entry(&format!("e{i}"), "cmd", i)).unwrap();
        }
        let entries = list(&conn, 3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_list_empty() {
        let conn = test_conn();
        let entries = list(&conn, 10).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_all_returns_every_row() {
        let conn = test_conn();
        for i in 0..20 {
            insert(&conn, &sample_entry(&format!("e{i}"), "cmd", i)).unwrap();
        }
        let entries = list_all(&conn).unwrap();
        assert_eq!(entries.len(), 20);
    }

    #[test]
    fn test_search_finds_matching() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "git push",      100)).unwrap();
        insert(&conn, &sample_entry("b", "cargo build",   200)).unwrap();
        insert(&conn, &sample_entry("c", "git commit -m", 150)).unwrap();

        let entries = search(&conn, "git", 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.id == "a"));
        assert!(entries.iter().any(|e| e.id == "c"));
    }

    #[test]
    fn test_search_case_insensitive() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "GIT PUSH", 100)).unwrap();
        insert(&conn, &sample_entry("b", "npm run",  200)).unwrap();

        let entries = search(&conn, "git", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a");
    }

    #[test]
    fn test_search_no_match() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "git push", 100)).unwrap();
        let entries = search(&conn, "zzz", 10).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_insert_duplicate_id_ignored() {
        let conn = test_conn();
        let e1 = sample_entry("dup", "first",  100);
        let e2 = sample_entry("dup", "second", 200);
        insert(&conn, &e1).unwrap();
        insert(&conn, &e2).unwrap();

        let entries = list(&conn, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "first");
    }

    #[test]
    fn test_search_ordered_by_timestamp_desc() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "git push",   100)).unwrap();
        insert(&conn, &sample_entry("b", "git commit", 300)).unwrap();
        insert(&conn, &sample_entry("c", "git log",    200)).unwrap();

        let entries = search(&conn, "git", 10).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, "b");
        assert_eq!(entries[1].id, "c");
        assert_eq!(entries[2].id, "a");
    }

    #[test]
    fn test_search_literal_percent() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "find . -name '%.rs'", 100)).unwrap();
        insert(&conn, &sample_entry("b", "npm install",         200)).unwrap();

        let entries = search(&conn, "%.rs", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a");
    }

    #[test]
    fn test_search_literal_underscore() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "git checkout _main", 100)).unwrap();
        insert(&conn, &sample_entry("b", "git checkout main",  200)).unwrap();

        let entries = search(&conn, "_main", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a");
    }

    #[test]
    fn test_search_literal_exclamation() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "echo 'hello!'", 100)).unwrap();
        insert(&conn, &sample_entry("b", "echo 'hello'",  200)).unwrap();

        let entries = search(&conn, "hello!", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a");
    }

    #[test]
    fn test_search_failed_exit_code_entry() {
        let conn = test_conn();
        let mut e = sample_entry("a", "cargo test", 100);
        e.exit_code = Some(1);
        insert(&conn, &e).unwrap();

        let results = search(&conn, "cargo", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].exit_code, Some(1));
    }
}
