use anyhow::Result;
use rusqlite::{Connection, params};
use crate::models::Entry;

/// Open or create the SQLite history database, applying migrations.
pub fn open() -> Result<Connection> {
    let path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hx")
        .join("history.db");
    std::fs::create_dir_all(path.parent().unwrap())?;
    let conn = Connection::open(&path)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Run database schema migrations (idempotent).
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS history (
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
        CREATE INDEX IF NOT EXISTS idx_cmd ON history(command);
    ")?;
    Ok(())
}

/// Insert an entry into the history table. Silently ignores duplicates.
pub fn insert(conn: &Connection, e: &Entry) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO history
         (id, command, cwd, exit_code, duration_ms, session_id, hostname, timestamp)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![e.id, e.command, e.cwd, e.exit_code, e.duration_ms,
                e.session_id, e.hostname, e.timestamp],
    )?;
    Ok(())
}

/// Search history for commands matching `query` (substring match), ordered by
/// most recent first. `%` and `_` in the query are treated as literal characters.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Entry>> {
    let escaped = query
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_");
    let pattern = format!("%{}%", escaped);
    let mut stmt = conn.prepare(
        "SELECT id,command,cwd,exit_code,duration_ms,session_id,hostname,timestamp
         FROM history
         WHERE command LIKE ?1 ESCAPE '!'
         ORDER BY timestamp DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pattern, limit as i64], row_to_entry)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// List the most recent history entries, ordered by timestamp descending.
pub fn list(conn: &Connection, limit: usize) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare(
        "SELECT id,command,cwd,exit_code,duration_ms,session_id,hostname,timestamp
         FROM history ORDER BY timestamp DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], row_to_entry)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
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
    fn test_search_finds_matching() {
        let conn = test_conn();
        insert(&conn, &sample_entry("a", "git push",       100)).unwrap();
        insert(&conn, &sample_entry("b", "cargo build",    200)).unwrap();
        insert(&conn, &sample_entry("c", "git commit -m",  150)).unwrap();

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
}
