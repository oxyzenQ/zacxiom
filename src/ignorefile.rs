// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `.zacxiomignore` file support — project-level exclude patterns.
//!
//! Like `.gitignore`, a `.zacxiomignore` file in a directory being scanned
//! adds exclude patterns scoped to that directory and its subdirectories.
//!
//! Multiple `.zacxiomignore` files can exist at different levels of the
//! directory tree; their patterns accumulate.
//!
//! Example file content:
//! ```text
//! # Protect my VM images
//! *.vmdk
//! *.vdi
//!
//! # Protect the entire backups/ subdirectory
//! /backups/
//!
//! # Protect .env files
//! .env*
//! ```

use crate::exclude::ExcludeFilter;
use globset::{Glob, GlobSetBuilder};
use std::fs;
use std::path::{Path, PathBuf};

const IGNORE_FILE_NAME: &str = ".zacxiomignore";

/// Discover and load all `.zacxiomignore` files within a scan root.
///
/// Walks the directory tree up to 5 levels deep, collecting ignore files.
/// Returns a list of (directory_of_ignore_file, patterns) pairs.
pub fn discover(root: &Path) -> Vec<(PathBuf, Vec<String>)> {
    let mut found = Vec::new();
    collect_ignore_files(root, &mut found, 0);
    found
}

fn collect_ignore_files(dir: &Path, found: &mut Vec<(PathBuf, Vec<String>)>, depth: usize) {
    if depth > 5 {
        return;
    }
    let ignore_path = dir.join(IGNORE_FILE_NAME);
    if ignore_path.is_file() {
        if let Ok(contents) = fs::read_to_string(&ignore_path) {
            let patterns: Vec<String> = contents
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| l.to_string())
                .collect();
            if !patterns.is_empty() {
                found.push((dir.to_path_buf(), patterns));
            }
        }
    }

    // Recurse into subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    // Skip hidden directories (e.g. .git, .svn)
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if !name_str.starts_with('.') {
                        collect_ignore_files(&entry.path(), found, depth + 1);
                    }
                }
            }
        }
    }
}

/// Build an ExcludeFilter from all discovered `.zacxiomignore` files.
///
/// Each pattern is compiled as a glob. Patterns are scoped to the directory
/// containing the ignore file — they match paths under that directory.
pub fn build_filter(ignore_files: &[(PathBuf, Vec<String>)]) -> Result<IgnoreFilter, String> {
    let mut builder = GlobSetBuilder::new();
    let mut scoped: Vec<(PathBuf, globset::Glob)> = Vec::new();

    for (dir, patterns) in ignore_files {
        for pat in patterns {
            let glob = Glob::new(pat).map_err(|e| {
                format!(
                    "Invalid pattern \"{pat}\" in {}: {e}",
                    dir.join(IGNORE_FILE_NAME).display()
                )
            })?;
            builder.add(glob.clone());
            scoped.push((dir.clone(), glob));
        }
    }

    let glob_set = builder.build().map_err(|e| format!("GlobSet build: {e}"))?;

    Ok(IgnoreFilter { scoped, glob_set })
}

/// Compiled `.zacxiomignore` filter with directory-scoped patterns.
pub struct IgnoreFilter {
    scoped: Vec<(PathBuf, globset::Glob)>,
    glob_set: globset::GlobSet,
}

impl IgnoreFilter {
    /// Returns true if the path matches any `.zacxiomignore` rule.
    /// A rule only matches if the path is under the directory containing the ignore file.
    pub fn is_ignored(&self, path: &Path) -> bool {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        for (dir, _glob) in &self.scoped {
            // Path must be under this ignore file's directory
            let dir_canon = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.clone());
            if !canon.starts_with(&dir_canon) && !path.starts_with(dir) {
                continue;
            }
            // Match the glob against the path (full path or filename)
            if self.glob_set.is_match(path) {
                return true;
            }
            if let Some(name) = path.file_name() {
                if self.glob_set.is_match(name) {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if no ignore rules exist.
    pub fn is_empty(&self) -> bool {
        self.scoped.is_empty()
    }
}

/// Convenience: discover + build in one call for a scan root.
pub fn load_for_root(root: &Path) -> IgnoreFilter {
    let files = discover(root);
    match build_filter(&files) {
        Ok(f) => f,
        Err(_) => IgnoreFilter {
            scoped: Vec::new(),
            glob_set: globset::GlobSet::empty(),
        },
    }
}

/// Combined exclusion check: config + CLI + .zacxiomignore
pub fn is_excluded(
    path: &Path,
    config_filter: &ExcludeFilter,
    ignore_filter: &IgnoreFilter,
) -> bool {
    if config_filter.is_excluded(path) {
        return true;
    }
    if !ignore_filter.is_empty() && ignore_filter.is_ignored(path) {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_ignore_file_discovered() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(IGNORE_FILE_NAME),
            "*.iso\n# comment\n*.vmdk\n",
        )
        .unwrap();

        let found = discover(tmp.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, vec!["*.iso", "*.vmdk"]);
    }

    #[test]
    fn test_nested_ignore_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(IGNORE_FILE_NAME), "*.iso\n").unwrap();
        fs::create_dir_all(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub").join(IGNORE_FILE_NAME), "*.tmp\n").unwrap();

        let found = discover(tmp.path());
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_ignore_filter_matches() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(IGNORE_FILE_NAME), "*.iso\n/backups/\n").unwrap();

        let files = discover(tmp.path());
        let filter = build_filter(&files).unwrap();

        let iso_path = tmp.path().join("ubuntu.iso");
        fs::write(&iso_path, b"data").unwrap();
        assert!(filter.is_ignored(&iso_path));
    }

    #[test]
    fn test_empty_filter() {
        let filter = IgnoreFilter {
            scoped: Vec::new(),
            glob_set: globset::GlobSet::empty(),
        };
        assert!(filter.is_empty());
    }

    #[test]
    fn test_hidden_dirs_skipped() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".git").join(IGNORE_FILE_NAME), "*.iso\n").unwrap();

        let found = discover(tmp.path());
        assert_eq!(found.len(), 0); // .git directory is skipped
    }
}
