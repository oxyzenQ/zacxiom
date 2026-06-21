// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Process awareness (procfs) — detect files open by running processes.
//!
//! Reads /proc to find file descriptors.
//! Used by the risk engine to protect actively-used files.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Set of paths currently open by any process.
#[allow(dead_code)]
pub fn open_files() -> HashSet<PathBuf> {
    let mut open = HashSet::new();

    let entries = match fs::read_dir("/proc") {
        Ok(e) => e,
        Err(_) => return open,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        // Only process directories (numeric pids)
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        // Scan /proc/<pid>/fd/
        let fd_dir = path.join("fd");
        if let Ok(fds) = fs::read_dir(&fd_dir) {
            for fd in fds.flatten() {
                if let Ok(target) = fs::read_link(fd.path()) {
                    let resolved = resolve_proc_path(&target);
                    open.insert(resolved);
                }
            }
        }
    }

    open
}

/// Check if a specific file is currently open by any running process.
#[allow(dead_code)]
pub fn is_file_open(path: &Path, open_files: &HashSet<PathBuf>) -> bool {
    open_files.contains(path)
}

/// Resolve a /proc symlink target to a canonical path.
#[allow(dead_code)]
fn resolve_proc_path(target: &Path) -> PathBuf {
    // If the target exists, canonicalize it; otherwise use as-is
    if target.exists() {
        target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf())
    } else {
        target.to_path_buf()
    }
}

/// Scan for files open by processes, excluding protected system paths.
/// Returns a HashSet for fast lookup.
pub fn build_open_file_set() -> HashSet<PathBuf> {
    let mut set = HashSet::new();

    let entries = match fs::read_dir("/proc") {
        Ok(e) => e,
        Err(_) => return set,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let fd_dir = path.join("fd");
        if let Ok(fds) = fs::read_dir(&fd_dir) {
            for fd in fds.flatten() {
                if let Ok(target) = fs::read_link(fd.path()) {
                    set.insert(target);
                }
            }
        }
    }

    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_open_file_set() {
        let set = build_open_file_set();
        // In a test environment, we should find at least some open files
        // (the test process itself has open fds)
        // This test just verifies the function doesn't panic
        assert!(!set.is_empty() || cfg!(not(target_os = "linux")));
    }

    #[test]
    fn test_is_file_open_false_for_nonexistent() {
        let set = build_open_file_set();
        assert!(!is_file_open(Path::new("/nonexistent/file/xyzzy"), &set));
    }
}
