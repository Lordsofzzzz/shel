use serde::{Deserialize, Serialize};

/// A single shell history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub command: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub session_id: Option<String>,
    pub hostname: Option<String>,
    /// Unix timestamp in milliseconds (UTC).
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_creation() {
        let e = Entry {
            id: "abc".into(),
            command: "git push".into(),
            cwd: Some("/home/user".into()),
            exit_code: Some(0),
            duration_ms: Some(300),
            session_id: Some("s1".into()),
            hostname: Some("host".into()),
            timestamp: 1_000_000,
        };
        assert_eq!(e.id, "abc");
        assert_eq!(e.command, "git push");
        assert_eq!(e.cwd.as_deref(), Some("/home/user"));
        assert_eq!(e.exit_code, Some(0));
        assert_eq!(e.duration_ms, Some(300));
        assert_eq!(e.session_id.as_deref(), Some("s1"));
        assert_eq!(e.hostname.as_deref(), Some("host"));
        assert_eq!(e.timestamp, 1_000_000);
    }

    #[test]
    fn test_entry_clone() {
        let e = Entry {
            id: "x".into(),
            command: "ls".into(),
            cwd: None,
            exit_code: Some(0),
            duration_ms: None,
            session_id: None,
            hostname: None,
            timestamp: 0,
        };
        let c = e.clone();
        assert_eq!(c.id, e.id);
        assert_eq!(c.command, e.command);
    }

    #[test]
    fn test_entry_serde_roundtrip() {
        let e = Entry {
            id: "xyz".into(),
            command: "cargo build".into(),
            cwd: None,
            exit_code: Some(1),
            duration_ms: None,
            session_id: None,
            hostname: None,
            timestamp: 2_000_000,
        };
        let json = serde_json::to_string(&e).unwrap();
        let de: Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(de.id, e.id);
        assert_eq!(de.command, e.command);
        assert!(de.cwd.is_none());
        assert_eq!(de.exit_code, Some(1));
        assert!(de.duration_ms.is_none());
        assert!(de.session_id.is_none());
        assert!(de.hostname.is_none());
        assert_eq!(de.timestamp, 2_000_000);
    }

    #[test]
    fn test_entry_serde_all_none() {
        let e = Entry {
            id: "".into(),
            command: "".into(),
            cwd: None,
            exit_code: None,
            duration_ms: None,
            session_id: None,
            hostname: None,
            timestamp: 0,
        };
        let json = serde_json::to_string(&e).unwrap();
        let de: Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(de.id, "");
        assert_eq!(de.timestamp, 0);
    }

    #[test]
    fn test_entry_failed_exit_code() {
        let e = Entry {
            id: "f1".into(),
            command: "cargo test".into(),
            cwd: None,
            exit_code: Some(101),
            duration_ms: Some(5_000),
            session_id: None,
            hostname: None,
            timestamp: 999,
        };
        assert_eq!(e.exit_code, Some(101));
        assert!(e.exit_code.map(|c| c != 0).unwrap_or(false));
    }
}
