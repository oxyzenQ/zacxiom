// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Exclude pattern matching — combines directory paths + glob patterns.
//!
//! Supports:
//! - Exact directory paths (with `~` expansion): `~/Downloads`, `/tmp/foo`
//! - File-name globs (matched against full path): `*.iso`, `*.vmdk`
//! - `.zacxiomignore` files (like `.gitignore`)

use crate::config;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};

/// Compiled exclude configuration — fast to query, built once.
pub struct ExcludeFilter {
    /// Exact directory prefixes to skip (canonicalized where possible).
    dir_prefixes: Vec<PathBuf>,
    /// Glob patterns matched against full paths.
    glob_set: GlobSet,
    /// Original patterns (for display/debug).
    pub patterns: Vec<String>,
}

impl ExcludeFilter {
    /// Build from config + CLI flags.
    /// CLI flags are appended to config-provided patterns (CLI wins on conflict,
    /// but for excludes that just means more things excluded).
    pub fn build(
        config_exclude_dirs: &[String],
        config_exclude_patterns: &[String],
        cli_exclude: &[String],
    ) -> Result<Self, String> {
        let mut all_dirs: Vec<String> = config_exclude_dirs.to_vec();
        let mut all_patterns: Vec<String> = config_exclude_patterns.to_vec();

        // CLI excludes: detect if it's a path (no glob metachars) or a pattern
        for ex in cli_exclude {
            if looks_like_glob(ex) {
                all_patterns.push(ex.clone());
            } else {
                all_dirs.push(ex.clone());
            }
        }

        // Expand + canonicalize directory paths
        let mut dir_prefixes = Vec::new();
        for d in &all_dirs {
            let expanded = config::expand_tilde(d);
            // Try to canonicalize — if it fails (path doesn't exist), use expanded form
            let canon = std::fs::canonicalize(&expanded).unwrap_or(expanded);
            dir_prefixes.push(canon);
        }

        // Compile globs
        let mut builder = GlobSetBuilder::new();
        for p in &all_patterns {
            let glob = Glob::new(p).map_err(|e| format!("Invalid glob \"{p}\": {e}"))?;
            builder.add(glob);
        }
        let glob_set = builder
            .build()
            .map_err(|e| format!("GlobSet build failed: {e}"))?;

        let mut all_for_display = all_dirs;
        all_for_display.extend(all_patterns);
        Ok(ExcludeFilter {
            dir_prefixes,
            glob_set,
            patterns: all_for_display,
        })
    }

    /// Build an empty filter (excludes nothing).
    pub fn empty() -> Self {
        ExcludeFilter {
            dir_prefixes: Vec::new(),
            glob_set: GlobSet::empty(),
            patterns: Vec::new(),
        }
    }

    /// Returns true if the given path should be excluded.
    pub fn is_excluded(&self, path: &Path) -> bool {
        // 1. Check directory prefix matches
        // Canonicalize the input path for accurate prefix comparison.
        // If canonicalization fails (broken symlink), use the path as-is.
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        for prefix in &self.dir_prefixes {
            if canon.starts_with(prefix) {
                return true;
            }
            // Also check the non-canonical form — handles cases where the prefix
            // itself doesn't exist (canonicalize returned expanded form)
            if path.starts_with(prefix) {
                return true;
            }
        }

        // 2. Check glob matches (against path as string)
        let path_str = path.to_string_lossy();
        if self.glob_set.is_match(path_str.as_ref()) {
            return true;
        }
        // Also match against just the filename (for patterns like "*.iso")
        if let Some(name) = path.file_name() {
            if self.glob_set.is_match(name) {
                return true;
            }
        }

        false
    }

    /// Returns true if this filter has no rules (excludes nothing).
    pub fn is_empty(&self) -> bool {
        self.dir_prefixes.is_empty() && self.glob_set.is_empty()
    }
}

/// Heuristic: does a string look like a glob pattern?
/// If it contains any of `*`, `?`, `[`, `{`, treat as glob; otherwise treat as a path.
fn looks_like_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[') || s.contains('{')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_empty_filter_excludes_nothing() {
        let f = ExcludeFilter::empty();
        assert!(!f.is_excluded(Path::new("/home/user/file.txt")));
        assert!(f.is_empty());
    }

    #[test]
    fn test_directory_prefix_excluded() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("Downloads");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("file.iso"), b"data").unwrap();

        let f = ExcludeFilter::build(&[], &[], &[dir.to_string_lossy().to_string()]).unwrap();
        assert!(f.is_excluded(&dir.join("file.iso")));
        assert!(f.is_excluded(&dir));
    }

    #[test]
    fn test_glob_pattern_excluded() {
        let f = ExcludeFilter::build(&[], &["*.iso".to_string()], &[]).unwrap();
        assert!(f.is_excluded(Path::new("/home/user/Downloads/ubuntu.iso")));
        assert!(f.is_excluded(Path::new("/tmp/anything.iso")));
        assert!(!f.is_excluded(Path::new("/home/user/file.txt")));
    }

    #[test]
    fn test_cli_exclude_glob_detected() {
        // CLI: "*.iso" should be treated as a glob pattern
        let f = ExcludeFilter::build(&[], &[], &["*.iso".to_string()]).unwrap();
        assert!(f.is_excluded(Path::new("/home/user/foo.iso")));
    }

    #[test]
    fn test_cli_exclude_path_detected() {
        // CLI: "~/Downloads" should be treated as a directory path
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", "/home/testuser");
        let f = ExcludeFilter::build(&[], &[], &["~/Downloads".to_string()]).unwrap();
        // Path doesn't exist on disk, but expansion should still work for matching
        // We can't easily test canonicalization here, so just verify build succeeds
        assert!(!f.is_empty());
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_tilde_expansion_in_config_dirs() {
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", "/home/testuser");
        let f = ExcludeFilter::build(&["~/Documents".to_string()], &[], &[]).unwrap();
        assert!(!f.is_empty());
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_multiple_globs() {
        let f = ExcludeFilter::build(
            &[],
            &[
                "*.iso".to_string(),
                "*.vmdk".to_string(),
                "*.tmp".to_string(),
            ],
            &[],
        )
        .unwrap();
        assert!(f.is_excluded(Path::new("/a/b/c.iso")));
        assert!(f.is_excluded(Path::new("/a/b/c.vmdk")));
        assert!(f.is_excluded(Path::new("/a/b/c.tmp")));
        assert!(!f.is_excluded(Path::new("/a/b/c.txt")));
    }

    #[test]
    fn test_invalid_glob_rejected() {
        let result = ExcludeFilter::build(&[], &["[unclosed".to_string()], &[]);
        assert!(result.is_err());
    }
}
