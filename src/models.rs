use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id:          String,
    pub command:     String,
    pub cwd:         Option<String>,
    pub exit_code:   Option<i64>,
    pub duration_ms: Option<i64>,
    pub session_id:  Option<String>,
    pub hostname:    Option<String>,
    pub timestamp:   i64,  // Unix ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_creation() {
        let e = Entry {
            id:          "abc".into(),
            command:     "git push".into(),
            cwd:         Some("/home/user".into()),
            exit_code:   Some(0),
            duration_ms: Some(300),
            session_id:  Some("s1".into()),
            hostname:    Some("host".into()),
            timestamp:   1_000_000,
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
    fn test_entry_serde_roundtrip() {
        let e = Entry {
            id:          "xyz".into(),
            command:     "cargo build".into(),
            cwd:         None,
            exit_code:   Some(1),
            duration_ms: None,
            session_id:  None,
            hostname:    None,
            timestamp:   2_000_000,
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
            id:          "".into(),
            command:     "".into(),
            cwd:         None,
            exit_code:   None,
            duration_ms: None,
            session_id:  None,
            hostname:    None,
            timestamp:   0,
        };
        let json = serde_json::to_string(&e).unwrap();
        let de: Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(de.id, "");
        assert_eq!(de.timestamp, 0);
    }
}