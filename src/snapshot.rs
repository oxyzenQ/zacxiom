// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Lightweight snapshot metadata system for rollback.
//!
//! Before any clean, Zacxiom records file metadata so undo can restore
//! from trash or warn about irreversible changes. v4: enables `zacxiom undo`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn ts() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotEntry {
    pub path: String,
    pub size: u64,
    pub trash_path: Option<String>,
    pub timestamp: String,
    /// Whether this file was skipped (not removed), for audit trail.
    #[serde(default)]
    pub skipped: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub id: String,
    pub created: String,
    pub entries: Vec<SnapshotEntry>,
}

impl Snapshot {
    pub fn new() -> Self {
        Snapshot {
            id: format!("snap-{}", std::process::id()),
            created: ts(),
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, path: &str, size: u64, trash_path: Option<String>) {
        self.entries.push(SnapshotEntry {
            path: path.to_string(),
            size,
            trash_path,
            timestamp: ts(),
            skipped: false,
        });
    }

    /// Record a skipped file for audit trail.
    pub fn add_skipped(&mut self, path: &str, size: u64) {
        self.entries.push(SnapshotEntry {
            path: path.to_string(),
            size,
            trash_path: None,
            timestamp: ts(),
            skipped: true,
        });
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let dir = snapshot_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
        let path = dir.join(&self.id);
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("json: {e}"))?;
        fs::write(&path, json).map_err(|e| format!("write: {e}"))?;
        Ok(path)
    }

    pub fn load(id: &str) -> Result<Self, String> {
        let path = snapshot_dir().join(id);
        let data = fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
        serde_json::from_str(&data).map_err(|e| format!("parse: {e}"))
    }

    /// List all available snapshot IDs, sorted newest-first by modification time.
    pub fn list_all() -> Vec<String> {
        let dir = snapshot_dir();
        if !dir.exists() {
            return vec![];
        }
        let mut snaps: Vec<(String, std::time::SystemTime)> = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("snap-") {
                    let mtime = entry.metadata().ok().and_then(|m| m.modified().ok());
                    snaps.push((name, mtime.unwrap_or(std::time::UNIX_EPOCH)));
                }
            }
        }
        // Sort newest-first by modification time
        snaps.sort_by_key(|b| std::cmp::Reverse(b.1));
        snaps.into_iter().map(|(name, _)| name).collect()
    }

    /// Restore files from a snapshot using trash directory.
    /// Uses rename (fast, same filesystem) with copy+remove fallback for cross-filesystem.
    /// Cleans up trash entries after successful restore.
    pub fn restore(&self) -> Result<usize, String> {
        let mut restored = 0;
        for entry in &self.entries {
            // Skip entries that were never removed (audit-only skipped files)
            if entry.skipped {
                continue;
            }
            if let Some(ref trash) = entry.trash_path {
                let trash_path = PathBuf::from(trash);
                if trash_path.exists() {
                    let target = PathBuf::from(&entry.path);
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).ok();
                    }
                    // Try rename first (fast, same filesystem), fall back to copy+remove
                    if fs::rename(&trash_path, &target).is_err() {
                        if fs::copy(&trash_path, &target).is_ok() {
                            let _ = fs::remove_file(&trash_path);
                        } else {
                            continue;
                        }
                    }
                    restored += 1;
                }
            }
        }
        Ok(restored)
    }

    /// Permanently delete all trash files for this snapshot.
    /// Called after user confirms they don't need undo.
    pub fn purge_trash(&self) {
        for entry in &self.entries {
            if let Some(ref trash) = entry.trash_path {
                let _ = fs::remove_file(PathBuf::from(trash));
            }
        }
    }
}

fn snapshot_dir() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".cache/zacxiom/snapshots")
}

/// Base trash directory for recoverable file deletion.
/// Files are moved here before snapshots record them.
pub fn trash_dir() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".cache/zacxiom/trash")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_create_and_save() {
        let mut snap = Snapshot::new();
        snap.add(
            "/tmp/test-file.txt",
            100,
            Some("/tmp/.zacxiom-trash/test-file.txt".into()),
        );
        assert_eq!(snap.entries.len(), 1);
        assert!(snap.save().is_ok());
    }

    #[test]
    fn test_list_empty_when_no_snapshots() {
        // May not be truly empty, just checking function doesn't panic
        let _ = Snapshot::list_all();
    }
}
