//! Installation history: a small JSON log in the app data folder. No package contents,
//! certificates or passwords are stored — only what was installed where, and the outcome.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Unix seconds.
    pub time: i64,
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub version: Option<String>,
    pub file_name: String,
    pub tv_name: String,
    pub tv_ip: String,
    /// "success" | "failed" | "cancelled"
    pub result: String,
    pub error_code: Option<String>,
    /// Sanitised technical detail (already passed through the redactor).
    pub detail: Option<String>,
}

fn file(dir: &Path) -> PathBuf {
    dir.join("history.json")
}

pub fn load(dir: &Path) -> Vec<HistoryEntry> {
    std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Appends an entry (newest first) and trims the log.
pub fn append(dir: &Path, entry: HistoryEntry) -> std::io::Result<()> {
    let mut entries = load(dir);
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);
    let tmp = dir.join("history.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&entries)?)?;
    std::fs::rename(tmp, file(dir))
}

pub fn clear(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(file(dir)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: i64) -> HistoryEntry {
        HistoryEntry {
            time: n,
            app_name: Some(format!("App {n}")),
            app_id: None,
            version: None,
            file_name: "a.wgt".into(),
            tv_name: "TV".into(),
            tv_ip: "192.168.1.5".into(),
            result: "success".into(),
            error_code: None,
            detail: None,
        }
    }

    #[test]
    fn newest_first_capped_and_clearable() {
        let d = tempfile::tempdir().unwrap();
        assert!(load(d.path()).is_empty());
        for n in 0..(MAX_ENTRIES as i64 + 5) {
            append(d.path(), entry(n)).unwrap();
        }
        let all = load(d.path());
        assert_eq!(all.len(), MAX_ENTRIES);
        assert_eq!(all[0].time, MAX_ENTRIES as i64 + 4);
        clear(d.path()).unwrap();
        clear(d.path()).unwrap();
        assert!(load(d.path()).is_empty());
    }

    #[test]
    fn corrupt_file_is_treated_as_empty() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(file(d.path()), "[{oops").unwrap();
        assert!(load(d.path()).is_empty());
        append(d.path(), entry(1)).unwrap();
        assert_eq!(load(d.path()).len(), 1);
    }
}
