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
    /// Number of actually removed files (not skipped).
    pub fn entry_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.skipped).count()
    }

    /// Number of skipped files in this snapshot.
    pub fn skipped_count(&self) -> usize {
        self.entries.iter().filter(|e| e.skipped).count()
    }

    /// Creation timestamp.
    pub fn created(&self) -> Option<String> {
        if self.created.is_empty() {
            None
        } else {
            Some(self.created.clone())
        }
    }
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
        // v13: Try XDG dir first, then legacy for backward compat
        let xdg_path = snapshot_dir().join(id);
        let legacy_path = snapshot_dir_legacy().join(id);
        let path = if xdg_path.exists() {
            xdg_path
        } else if legacy_path.exists() {
            legacy_path
        } else {
            xdg_path // will produce a readable error
        };
        let data = fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
        serde_json::from_str(&data).map_err(|e| format!("parse: {e}"))
    }

    /// List all available snapshot IDs, sorted newest-first by modification time.
    /// Gracefully skips unreadable entries and broken symlinks.
    pub fn list_all() -> Vec<String> {
        // v13: Check both XDG and legacy directories for backward compatibility
        let dirs = [snapshot_dir(), snapshot_dir_legacy()];
        let mut snaps: Vec<(String, std::time::SystemTime)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for dir in &dirs {
            if !dir.exists() {
                continue;
            }
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("snap-") || seen.contains(&name) {
                    continue;
                }
                // Use metadata() safely — skip entries that fail (broken symlinks, permissions)
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::UNIX_EPOCH);
                seen.insert(name.clone());
                snaps.push((name, mtime));
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

/// Snapshot storage directory (XDG-compliant).
///
/// v13: Migrated from ~/.cache/zacxiom/snapshots to ~/.local/share/zacxiom/snapshots
/// per XDG Base Directory Spec (snapshots are user data, not disposable cache).
/// Respects XDG_DATA_HOME if set.
///
/// For backward compatibility, list_all() and load() also check the legacy
/// ~/.cache/zacxiom/snapshots location.
pub fn snapshot_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("zacxiom/snapshots")
    } else {
        let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
        PathBuf::from(home).join(".local/share/zacxiom/snapshots")
    }
}

/// Legacy snapshot directory (pre-v13). Used for backward-compatible reads.
pub fn snapshot_dir_legacy() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".cache/zacxiom/snapshots")
}

/// Base trash directory for recoverable file deletion.
/// Files are moved here before snapshots record them.
/// v13: XDG-compliant location.
pub fn trash_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("zacxiom/trash")
    } else {
        let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
        PathBuf::from(home).join(".local/share/zacxiom/trash")
    }
}

/// Delete a snapshot by ID (metadata file only, not trash files).
/// v13: Checks both XDG and legacy directories.
pub fn delete_snapshot(id: &str) -> Result<(), String> {
    let xdg_path = snapshot_dir().join(id);
    let legacy_path = snapshot_dir_legacy().join(id);
    if xdg_path.exists() {
        std::fs::remove_file(&xdg_path).map_err(|e| format!("Cannot delete {id}: {e}"))
    } else if legacy_path.exists() {
        std::fs::remove_file(&legacy_path).map_err(|e| format!("Cannot delete {id}: {e}"))
    } else {
        Err(format!("Snapshot {id} not found"))
    }
}

impl Snapshot {
    /// Delete this snapshot's metadata file.
    pub fn delete(id: &str) -> Result<(), String> {
        delete_snapshot(id)
    }

    /// Calculate total size of this snapshot in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| !e.skipped)
            .map(|e| e.size)
            .sum()
    }
}

/// Get human-readable age string for a snapshot.
pub fn snapshot_age(id: &str) -> String {
    let secs = snapshot_age_secs(id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age_secs = now.saturating_sub(secs);

    if age_secs < 60 {
        format!("{}s ago", age_secs)
    } else if age_secs < 3600 {
        format!("{}m ago", age_secs / 60)
    } else if age_secs < 86400 {
        format!("{}h ago", age_secs / 3600)
    } else {
        let days = age_secs / 86400;
        if days < 7 {
            format!("{}d ago", days)
        } else {
            format!("{}w ago", days / 7)
        }
    }
}

/// Get snapshot creation timestamp as seconds since epoch.
/// v13: Checks both XDG and legacy directories.
pub fn snapshot_age_secs(id: &str) -> u64 {
    // Try XDG first, then legacy
    for dir in &[snapshot_dir(), snapshot_dir_legacy()] {
        let path = dir.join(id);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(snap) = serde_json::from_str::<Snapshot>(&data) {
                return snap.created.parse::<u64>().unwrap_or(0);
            }
        }
        // Fallback: use file mtime
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(mtime) = meta.modified() {
                return mtime
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
            }
        }
    }
    0
}

/// Calculate total storage used by all snapshots.
pub fn total_snapshot_size() -> (usize, u64) {
    let all = Snapshot::list_all();
    let mut total: u64 = 0;
    for id in &all {
        if let Ok(snap) = Snapshot::load(id) {
            total += snap.total_size_bytes();
        }
    }
    (all.len(), total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// RAII guard that restores HOME env var on drop.
    struct HomeGuard(Option<std::ffi::OsString>);
    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(home) => env::set_var("HOME", home),
                None => env::remove_var("HOME"),
            }
        }
    }

    #[test]
    fn test_snapshot_create_and_save() {
        // Use a temp dir as HOME so the test doesn't depend on
        // ~/.cache/zacxiom/snapshots/ being writable (stale root-owned dirs).
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = HomeGuard(env::var_os("HOME"));
        env::set_var("HOME", tmp.path());

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
