use shel::db;
use shel::models::Entry;
use rusqlite::Connection;

fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::migrate(&conn).unwrap();
    conn
}

fn entry(id: &str, cmd: &str, ts: i64) -> Entry {
    Entry {
        id: id.into(),
        command: cmd.into(),
        cwd: Some("/home/user".into()),
        exit_code: Some(0),
        duration_ms: None,
        session_id: None,
        hostname: None,
        timestamp: ts,
    }
}

#[test]
fn test_insert_and_search_roundtrip() {
    let conn = test_db();
    db::insert(&conn, &entry("a", "git push", 100)).unwrap();
    db::insert(&conn, &entry("b", "cargo build", 200)).unwrap();

    let results = db::search(&conn, "git", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].command, "git push");
}

#[test]
fn test_insert_and_list_roundtrip() {
    let conn = test_db();
    db::insert(&conn, &entry("a", "git push", 100)).unwrap();
    db::insert(&conn, &entry("b", "cargo build", 200)).unwrap();

    let results = db::list(&conn, 10).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_search_special_chars_are_literal() {
    let conn = test_db();
    db::insert(&conn, &entry("a", "find . -name '%.rs'", 100)).unwrap();
    db::insert(&conn, &entry("b", "git checkout _main", 200)).unwrap();

    let r1 = db::search(&conn, "%.rs", 10).unwrap();
    assert_eq!(r1.len(), 1, "% should not act as LIKE wildcard");
    assert_eq!(r1[0].id, "a");

    let r2 = db::search(&conn, "_main", 10).unwrap();
    assert_eq!(r2.len(), 1, "_ should not act as LIKE wildcard");
    assert_eq!(r2[0].id, "b");
}

#[test]
fn test_search_pagination() {
    let conn = test_db();
    for i in 0..10 {
        let id = format!("e{i}");
        db::insert(&conn, &entry(&id, "cmd", i)).unwrap();
    }

    let all = db::list(&conn, 100).unwrap();
    assert_eq!(all.len(), 10);

    let limited = db::list(&conn, 3).unwrap();
    assert_eq!(limited.len(), 3);
}

#[test]
fn test_empty_db() {
    let conn = test_db();
    assert!(db::list(&conn, 10).unwrap().is_empty());
    assert!(db::search(&conn, "anything", 10).unwrap().is_empty());
}

#[test]
fn test_duplicate_insert_idempotent() {
    let conn = test_db();
    let e = entry("dup", "first", 100);
    db::insert(&conn, &e).unwrap();
    db::insert(&conn, &e).unwrap();

    let results = db::list(&conn, 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_search_ordering() {
    let conn = test_db();
    db::insert(&conn, &entry("a", "git push",   100)).unwrap();
    db::insert(&conn, &entry("b", "git commit", 300)).unwrap();
    db::insert(&conn, &entry("c", "npm install", 200)).unwrap();

    let results = db::search(&conn, "git", 10).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "b", "most recent first");
    assert_eq!(results[1].id, "a", "older second");
}
