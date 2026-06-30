// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! File discovery engine.
//!
//! Walks the filesystem from a given root, collecting regular files
//! with their sizes. Excludes protected paths and respects depth limits.
//!
//! v13: Excludes user-content directories from default roots (Downloads removed
//! to prevent accidental deletion of ISOs and user files). Default scan is now
//! strictly cache/dev directories. User must explicitly opt-in to scan user dirs.

use crate::exclude::ExcludeFilter;
use crate::ignorefile;
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
/// - `exclude`: v13 exclude filter (config + CLI + .zacxiomignore)
///   Pass `&ExcludeFilter::empty()` to disable.
pub fn scan(
    roots: &[PathBuf],
    max_depth: usize,
    min_size_bytes: u64,
    skip_protected: bool,
    exclude: &ExcludeFilter,
) -> Vec<ScanEntry> {
    let mut entries = Vec::new();

    for root in roots {
        if !root.exists() {
            continue;
        }

        // v13: Load .zacxiomignore files for this root
        let ignore = ignorefile::load_for_root(root);

        let walker = if max_depth > 0 {
            WalkDir::new(root).max_depth(max_depth).follow_links(false)
        } else {
            WalkDir::new(root).follow_links(false)
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

            // v13: skip excluded paths (config + CLI + .zacxiomignore)
            if (!exclude.is_empty() || !ignore.is_empty())
                && ignorefile::is_excluded(&path, exclude, &ignore)
            {
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
///
/// v13: ~/Downloads REMOVED from defaults — user files (ISOs, installers,
/// documents) were being accidentally deleted. Users who want to scan
/// Downloads must pass it explicitly: `zacxiom scan ~/Downloads`.
/// Better yet, use `zacxiom plan ~/Downloads` (read-only preview).
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
        // v13: ~/Downloads REMOVED — too dangerous in defaults.

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

/// v13: Check if a path is inside a user-content directory (Downloads, Documents, etc.)
/// Used to emit warnings before scanning.
pub fn is_user_content_dir(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    path_str.contains("/downloads")
        || path_str.contains("/documents")
        || path_str.contains("/pictures")
        || path_str.contains("/videos")
        || path_str.contains("/music")
        || path_str.contains("/desktop")
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
        let entries = scan(
            &[tmp.path().to_path_buf()],
            0,
            0,
            false,
            &ExcludeFilter::empty(),
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn test_scan_finds_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        fs::write(tmp.path().join("b.txt"), b"world!").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/c.txt"), b"nested").unwrap();

        let entries = scan(
            &[tmp.path().to_path_buf()],
            0,
            0,
            false,
            &ExcludeFilter::empty(),
        );
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_scan_respects_min_size() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("small.txt"), b"x").unwrap();
        fs::write(tmp.path().join("big.txt"), [0u8; 100]).unwrap();

        let entries = scan(
            &[tmp.path().to_path_buf()],
            0,
            50,
            false,
            &ExcludeFilter::empty(),
        );
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
        let all = scan(
            &[tmp.path().to_path_buf()],
            4,
            0,
            false,
            &ExcludeFilter::empty(),
        );
        assert_eq!(all.len(), 4);

        // max_depth 1: only root.txt (depth 0) — walkdir max_depth=1 means 1 level of directories below root
        let shallow = scan(
            &[tmp.path().to_path_buf()],
            1,
            0,
            false,
            &ExcludeFilter::empty(),
        );
        assert_eq!(shallow.len(), 1);
        assert!(shallow[0].path.ends_with("root.txt"));
    }

    #[test]
    fn test_scan_respects_exclude_filter() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("keep.txt"), b"keep").unwrap();
        fs::write(tmp.path().join("skip.iso"), b"skip").unwrap();

        let filter = ExcludeFilter::build(&[], &["*.iso".to_string()], &[]).unwrap();
        let entries = scan(&[tmp.path().to_path_buf()], 0, 0, false, &filter);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.ends_with("keep.txt"));
    }

    #[test]
    fn test_scan_respects_exclude_directory() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("keep.txt"), b"keep").unwrap();
        let skip_dir = tmp.path().join("skipme");
        fs::create_dir_all(&skip_dir).unwrap();
        fs::write(skip_dir.join("secret.txt"), b"secret").unwrap();

        let filter =
            ExcludeFilter::build(&[], &[], &[skip_dir.to_string_lossy().to_string()]).unwrap();
        let entries = scan(&[tmp.path().to_path_buf()], 0, 0, false, &filter);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.ends_with("keep.txt"));
    }

    #[test]
    fn test_default_scan_roots_excludes_downloads() {
        // v13 regression: ~/Downloads must NOT be in default roots
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", "/home/testuser");
        let roots = default_scan_roots();
        let any_downloads = roots
            .iter()
            .any(|r| r.to_string_lossy().contains("Downloads"));
        assert!(
            !any_downloads,
            "~/Downloads must not be in default scan roots"
        );
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_is_user_content_dir() {
        assert!(is_user_content_dir(std::path::Path::new(
            "/home/user/Downloads"
        )));
        assert!(is_user_content_dir(std::path::Path::new(
            "/home/user/Documents/report.pdf"
        )));
        assert!(!is_user_content_dir(std::path::Path::new(
            "/home/user/.cache"
        )));
    }
}
