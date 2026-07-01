// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Audit log (v13.3) — structured JSONL logs for every clean operation.
//!
//! Location: ~/.local/share/zacxiom/audit.log (XDG_DATA_HOME — user data, not cache)
//!
//! Each line is a self-contained JSON object representing one clean session:
//! ```json
//! {"ts":"2026-07-01T04:00:00Z","event":"clean","snapshot_id":"snap-...","files_removed":24068,"bytes_freed":716782336,"files_skipped":61837,"errors":10,"mode":"smart"}
//! ```
//!
//! This is append-only — never modified after writing. Designed for:
//! - Compliance auditing (who cleaned what, when)
//! - Debugging (what happened in the last clean)
//! - Analytics (frequency, space recovered over time)

use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// A single audit log entry.
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    /// ISO 8601 timestamp
    pub ts: String,
    /// Event type: "clean", "undo", "scan", "config_change"
    pub event: &'static str,
    /// Snapshot ID (for clean/undo events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    /// Files removed (clean events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_removed: Option<usize>,
    /// Bytes freed (clean events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_freed: Option<u64>,
    /// Files skipped (clean events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_skipped: Option<usize>,
    /// Error count (clean events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<usize>,
    /// Clean mode: "safe" | "smart" | "force"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<&'static str>,
    /// Files restored (undo events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_restored: Option<usize>,
    /// Files scanned (scan events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_scanned: Option<usize>,
    /// Exit code (for scripting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

impl AuditEntry {
    /// Create a clean event entry.
    pub fn clean(
        snapshot_id: &str,
        files_removed: usize,
        bytes_freed: u64,
        files_skipped: usize,
        errors: usize,
        mode: &'static str,
    ) -> Self {
        AuditEntry {
            ts: crate::pipeline::chrono_now(),
            event: "clean",
            snapshot_id: Some(snapshot_id.to_string()),
            files_removed: Some(files_removed),
            bytes_freed: Some(bytes_freed),
            files_skipped: Some(files_skipped),
            errors: Some(errors),
            mode: Some(mode),
            files_restored: None,
            files_scanned: None,
            exit_code: None,
        }
    }

    /// Create an undo event entry.
    pub fn undo(snapshot_id: &str, files_restored: usize) -> Self {
        AuditEntry {
            ts: crate::pipeline::chrono_now(),
            event: "undo",
            snapshot_id: Some(snapshot_id.to_string()),
            files_removed: None,
            bytes_freed: None,
            files_skipped: None,
            errors: None,
            mode: None,
            files_restored: Some(files_restored),
            files_scanned: None,
            exit_code: None,
        }
    }

    /// Create a scan event entry.
    pub fn scan(files_scanned: usize) -> Self {
        AuditEntry {
            ts: crate::pipeline::chrono_now(),
            event: "scan",
            snapshot_id: None,
            files_removed: None,
            bytes_freed: None,
            files_skipped: None,
            errors: None,
            mode: None,
            files_restored: None,
            files_scanned: Some(files_scanned),
            exit_code: None,
        }
    }

    /// Append this entry to the audit log (JSONL format).
    /// Best-effort — errors are silently ignored (audit log must never crash zacxiom).
    pub fn log(&self) {
        let path = audit_log_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
                let _ = writeln!(file, "{json}");
            }
        }
    }
}

/// Get audit log path: ~/.local/share/zacxiom/audit.log
fn audit_log_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("zacxiom/audit.log")
    } else {
        let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
        PathBuf::from(home).join(".local/share/zacxiom/audit.log")
    }
}

/// Get the audit log path (public, for `zacxiom config path` or status display).
pub fn audit_path() -> PathBuf {
    audit_log_path()
}

/// Read the last N entries from the audit log (for status display).
/// Returns parsed entries, newest-first.
pub fn read_recent(limit: usize) -> Vec<AuditEntryRead> {
    let path = audit_log_path();
    if !path.exists() {
        return Vec::new();
    }
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut entries: Vec<AuditEntryRead> = data
        .lines()
        .rev()
        .take(limit)
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    entries.reverse();
    entries
}

/// Readable version of AuditEntry (for status display).
#[derive(Debug, serde::Deserialize)]
pub struct AuditEntryRead {
    pub ts: String,
    pub event: String,
    pub snapshot_id: Option<String>,
    pub files_removed: Option<usize>,
    pub bytes_freed: Option<u64>,
    pub files_skipped: Option<usize>,
    pub errors: Option<usize>,
    pub mode: Option<String>,
    pub files_restored: Option<usize>,
    pub files_scanned: Option<usize>,
    pub exit_code: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_audit_entry_clean() {
        let entry = AuditEntry::clean("snap-test", 100, 1024, 50, 2, "smart");
        assert_eq!(entry.event, "clean");
        assert_eq!(entry.files_removed, Some(100));
        assert_eq!(entry.bytes_freed, Some(1024));
        assert_eq!(entry.mode, Some("smart"));
    }

    #[test]
    fn test_audit_entry_undo() {
        let entry = AuditEntry::undo("snap-test", 5);
        assert_eq!(entry.event, "undo");
        assert_eq!(entry.files_restored, Some(5));
    }

    #[test]
    fn test_audit_entry_scan() {
        let entry = AuditEntry::scan(85915);
        assert_eq!(entry.event, "scan");
        assert_eq!(entry.files_scanned, Some(85915));
    }

    #[test]
    fn test_audit_log_append() {
        let tmp = TempDir::new().unwrap();
        let old_home = std::env::var_os("HOME");
        let old_xdg = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("XDG_DATA_HOME");

        // Write two entries
        AuditEntry::clean("snap-1", 10, 100, 5, 0, "safe").log();
        AuditEntry::clean("snap-2", 20, 200, 10, 1, "smart").log();

        // Read them back
        let recent = read_recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].snapshot_id, Some("snap-1".into()));
        assert_eq!(recent[1].snapshot_id, Some("snap-2".into()));

        // Restore
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        match old_xdg {
            Some(h) => std::env::set_var("XDG_DATA_HOME", h),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }

    #[test]
    fn test_audit_log_json_serialization() {
        let entry = AuditEntry::clean("snap-test", 100, 1024, 50, 2, "smart");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"event\":\"clean\""));
        assert!(json.contains("\"files_removed\":100"));
        assert!(json.contains("\"mode\":\"smart\""));
        // Skipped fields should not be present
        assert!(!json.contains("files_restored"));
        assert!(!json.contains("files_scanned"));
    }
}
