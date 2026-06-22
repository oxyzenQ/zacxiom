// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! File discovery engine.
//!
//! Walks the filesystem from a given root, collecting regular files
//! with their sizes. Excludes protected paths and respects depth limits.

use crate::rules::is_protected;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Single scanned file entry (bare — no classification yet).
#[derive(Debug, Clone)]
pub struct ScanEntry {
    pub path: PathBuf,
    pub size: u64,
}

/// Scan a directory for regular files.
///
/// - `roots`: one or more starting paths
/// - `max_depth`: directory depth limit (0 = unlimited)
/// - `min_size_bytes`: skip files smaller than this
/// - `skip_protected`: if true, skip H2-protected paths entirely
pub fn scan(
    roots: &[PathBuf],
    max_depth: usize,
    min_size_bytes: u64,
    skip_protected: bool,
) -> Vec<ScanEntry> {
    let mut entries = Vec::new();

    for root in roots {
        if !root.exists() {
            continue;
        }

        let walker = if max_depth > 0 {
            WalkDir::new(root).max_depth(max_depth)
        } else {
            WalkDir::new(root)
        };

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();

            // skip dirs, symlinks, special files
            if !entry.file_type().is_file() {
                continue;
            }

            // skip protected paths (H2)
            if skip_protected && is_protected(&path) {
                continue;
            }

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

            // skip files below min size
            if size < min_size_bytes {
                continue;
            }

            entries.push(ScanEntry { path, size });
        }
    }

    entries
}

/// Default scan roots for a typical user cache scan.
pub fn default_scan_roots() -> Vec<PathBuf> {
    let home = dirs_home();
    let mut roots = vec![];

    if let Some(ref h) = home {
        // Developer caches — the biggest consumers on dev workstations
        roots.push(h.join(".cargo"));
        roots.push(h.join(".rustup"));
        roots.push(h.join(".npm"));
        roots.push(h.join(".docker"));
        roots.push(h.join(".gradle"));
        roots.push(h.join(".m2/repository"));

        // General cache & desktop
        roots.push(h.join(".cache"));
        roots.push(h.join(".local/share/Trash"));
        roots.push(h.join("Downloads"));

        // Gaming
        roots.push(h.join(".steam"));
        roots.push(h.join(".local/share/Steam"));
        roots.push(h.join(".var/app")); // Flatpak
        roots.push(h.join("snap"));
    }

    // system caches (readable by user)
    roots.push(PathBuf::from("/var/cache"));
    roots.push(PathBuf::from("/var/lib/docker"));
    roots.push(PathBuf::from("/tmp"));

    roots
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_scan_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let entries = scan(&[tmp.path().to_path_buf()], 0, 0, false);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_scan_finds_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        fs::write(tmp.path().join("b.txt"), b"world!").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/c.txt"), b"nested").unwrap();

        let entries = scan(&[tmp.path().to_path_buf()], 0, 0, false);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_scan_respects_min_size() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("small.txt"), b"x").unwrap();
        fs::write(tmp.path().join("big.txt"), [0u8; 100]).unwrap();

        let entries = scan(&[tmp.path().to_path_buf()], 0, 50, false);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.ends_with("big.txt"));
    }

    #[test]
    fn test_scan_respects_max_depth() {
        let tmp = TempDir::new().unwrap();
        // depth structure: root.txt (depth 0), sub1/ (depth 1), sub1/sub2/ (depth 2), sub1/sub2/sub3/ (depth 3)
        fs::create_dir_all(tmp.path().join("sub1/sub2/sub3")).unwrap();
        fs::write(tmp.path().join("root.txt"), b"r").unwrap();
        fs::write(tmp.path().join("sub1/a1.txt"), b"a1").unwrap();
        fs::write(tmp.path().join("sub1/sub2/b1.txt"), b"b1").unwrap();
        fs::write(tmp.path().join("sub1/sub2/sub3/c1.txt"), b"c1").unwrap();

        // max_depth 3: includes root.txt (depth 0), a1.txt (depth 1), b1.txt (depth 2), c1.txt (depth 3)
        let all = scan(&[tmp.path().to_path_buf()], 4, 0, false);
        assert_eq!(all.len(), 4);

        // max_depth 1: only root.txt (depth 0) — walkdir max_depth=1 means 1 level of directories below root
        let shallow = scan(&[tmp.path().to_path_buf()], 1, 0, false);
        assert_eq!(shallow.len(), 1);
        assert!(shallow[0].path.ends_with("root.txt"));
    }
}
