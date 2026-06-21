// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Clean history tracking.
//!
//! Records every clean operation for audit and future context awareness.
//! Stored in ~/.cache/zacxiom/history.json

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CleanRecord {
    pub timestamp: String,
    pub version: String,
    pub action: String,
    pub files_removed: usize,
    pub bytes_freed: u64,
    pub files_skipped: usize,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct History {
    pub records: Vec<CleanRecord>,
}

impl History {
    /// Load history from disk, or return empty if none exists.
    pub fn load() -> Self {
        let path = history_path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            History::default()
        }
    }

    /// Save history to disk.
    pub fn save(&self) {
        let path = history_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, json);
        }
    }

    /// Add a new clean record and save.
    pub fn add(&mut self, record: CleanRecord) {
        // Keep last 100 records max
        if self.records.len() >= 100 {
            self.records.remove(0);
        }
        self.records.push(record);
        self.save();
    }

    /// Get paths the user has cleaned before.
    pub fn previously_cleaned_paths(&self) -> Vec<String> {
        self.records.iter().flat_map(|r| r.paths.clone()).collect()
    }

    /// Count how many times a path has been cleaned.
    #[allow(dead_code)]
    pub fn clean_count(&self, path: &str) -> usize {
        self.records
            .iter()
            .filter(|r| r.paths.contains(&path.to_string()))
            .count()
    }
}

fn history_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".cache/zacxiom/history.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_empty() {
        let h = History::default();
        assert!(h.records.is_empty());
        assert!(h.previously_cleaned_paths().is_empty());
    }

    #[test]
    fn test_history_add() {
        let mut h = History::default();
        h.add(CleanRecord {
            timestamp: "2026-01-01T00:00:00Z".into(),
            version: "2.0.0".into(),
            action: "clean --smart".into(),
            files_removed: 3,
            bytes_freed: 1024,
            files_skipped: 1,
            paths: vec!["/tmp/a".into(), "/tmp/b".into()],
        });
        assert_eq!(h.records.len(), 1);
        assert_eq!(h.clean_count("/tmp/a"), 1);
        assert_eq!(h.previously_cleaned_paths().len(), 2);
    }
}
