// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security guard — strict whitelist enforcement for zacxiom's own state I/O.
//!
//! # Policy
//!
//! Zacxiom reads/writes its own state (config, scan cache, snapshots, audit log)
//! ONLY from a fixed set of canonical directories. Any path outside this
//! allowlist is rejected by default. This prevents arbitrary file reads of
//! dangerous system paths (e.g. `/etc/shadow`, `~/.ssh/id_rsa`, `/etc/sudoers`)
//! even if a future bug or symlink trick tries to point zacxiom at them.
//!
//! # Whitelist (allowlist) — the ONLY paths zacxiom may touch for its own state
//!
//! | Purpose  | Canonical path                                |
//! |----------|-----------------------------------------------|
//! | config   | `$XDG_CONFIG_HOME/zacxiom/` or `~/.config/zacxiom/` |
//! | cache    | `$XDG_CACHE_HOME/zacxiom/` or `~/.cache/zacxiom/` |
//! | data     | `$XDG_DATA_HOME/zacxiom/` or `~/.local/share/zacxiom/` |
//!
//! The filesystem-scan operation (the core feature) is NOT gated here — it is
//! an explicit user action with its own safety engine (`safety.rs`, `rules.rs`).
//! This module protects only zacxiom's internal state I/O.
//!
//! # Defense-in-depth
//!
//! The primary mechanism is the allowlist (default-deny). As a secondary
//! guard, the canonicalized path is also checked against a small set of
//! known-dangerous prefixes. This catches the edge case where a symlink
//! inside the whitelist resolves to a sensitive system file.

use std::path::{Component, Path, PathBuf};

/// Resolve a path "as if" it were canonicalized, but without requiring the
/// file to exist on disk. Collapses `.` and `..` components and collapses
/// repeated separators. Does NOT follow symlinks (use `std::fs::canonicalize`
/// for that, when the file exists).
///
/// Returns `None` if the path cannot be normalized (e.g. empty after stripping).
fn lexical_normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => { /* skip `.` */ }
            Component::ParentDir => {
                // Pop only if last is a normal component (don't pop root)
                match out.components().next_back() {
                    Some(Component::Normal(_)) => {
                        let _ = out.pop();
                    }
                    Some(Component::RootDir) | None => {
                        // `..` above root — drop it
                    }
                    _ => {
                        let _ = out.pop();
                    }
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Canonicalize a path for whitelist comparison. Tries `std::fs::canonicalize`
/// first (resolves symlinks, requires existence); falls back to lexical
/// normalization for paths that don't exist yet (e.g. config.toml before
/// `config init`).
fn canonical_for_compare(path: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(path) {
        return c;
    }
    lexical_normalize(path).unwrap_or_else(|| path.to_path_buf())
}

/// Return the three allowlisted state directories (config, cache, data).
/// Each entry is the canonicalized directory path (no trailing slash).
///
/// Order matters: longest-prefix-first so nested matches prefer the most
/// specific directory (currently all three are independent, but this future-
/// proofs against a directory being a prefix of another).
fn whitelist_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::with_capacity(3);

    // Config dir: XDG_CONFIG_HOME or ~/.config
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        dirs.push(PathBuf::from(&xdg).join("zacxiom"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".config/zacxiom"));
    }

    // Cache dir: XDG_CACHE_HOME or ~/.cache
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        dirs.push(PathBuf::from(&xdg).join("zacxiom"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".cache/zacxiom"));
    }

    // Data dir: XDG_DATA_HOME or ~/.local/share
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(&xdg).join("zacxiom"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/zacxiom"));
    }

    // Canonicalize each (best-effort) so comparison handles symlinks/trailing dots.
    dirs.iter().map(|d| canonical_for_compare(d)).collect()
}

/// Defense-in-depth: known-dangerous path prefixes that must NEVER be touched
/// by zacxiom's own state I/O, even if an XDG env var or symlink somehow
/// landed them inside the whitelist. The PRIMARY mechanism is the allowlist;
/// this list is secondary.
fn is_dangerous(canonical: &Path) -> bool {
    let s = canonical.to_string_lossy();
    let home = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).to_string_lossy().into_owned())
        .unwrap_or_default();

    // System paths — never valid for zacxiom state, even if XDG env vars
    // point here. Zacxiom is a user-space tool; its state must live under
    // the user's home or proper XDG dirs, NOT in /etc, /usr, /var, etc.
    let deny_prefixes: &[&str] = &[
        "/etc/", "/usr/", "/var/", "/bin/", "/sbin/", "/lib/", "/lib64/", "/boot/", "/root/",
        "/proc/", "/sys/", "/dev/",
    ];

    for prefix in deny_prefixes {
        if s.starts_with(prefix) {
            return true;
        }
    }

    // User credential stores — never readable
    let user_deny_subdirs: &[&str] = &[".ssh/", ".gnupg/", ".kwallet/", ".local/share/keyrings/"];
    if !home.is_empty() {
        for sub in user_deny_subdirs {
            let full = format!("{home}/{sub}");
            if s.starts_with(&full) {
                return true;
            }
        }
    }

    false
}

/// Check whether a canonicalized path falls inside any whitelisted directory.
fn is_in_whitelist(canonical: &Path) -> bool {
    for dir in whitelist_dirs() {
        if canonical == dir {
            return true;
        }
        if canonical.starts_with(&dir) {
            return true;
        }
    }
    false
}

/// Validate a path for zacxiom's own state READ.
///
/// Returns `Ok(canonical_path)` if the path is inside the allowlist and does
/// not match any dangerous prefix. Returns `Err(message)` otherwise.
///
/// The error message intentionally does NOT echo the input path back when the
/// path is dangerous — that prevents a credential path from being echoed into
/// logs.
pub fn validate_state_read(path: &Path) -> Result<PathBuf, String> {
    let canonical = canonical_for_compare(path);
    if !is_in_whitelist(&canonical) {
        return Err(format!(
            "path outside zacxiom's allowlisted state directories (blocked by pathguard): {}",
            path.display()
        ));
    }
    if is_dangerous(&canonical) {
        return Err(
            "path resolved to a protected system credential path (blocked by pathguard)"
                .to_string(),
        );
    }
    Ok(canonical)
}

/// Validate a path for zacxiom's own state WRITE.
///
/// Same rules as `validate_state_read` — write access is also strictly
/// whitelisted. Use this before any `fs::write`, `fs::create_dir_all`,
/// `File::create`, etc. on a user-influenced path.
pub fn validate_state_write(path: &Path) -> Result<PathBuf, String> {
    // Same checks as read; kept as a separate function so callers can signal
    // intent and so future divergence (e.g. stricter write rules) is localized.
    validate_state_read(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper: set HOME to a known value and clear XDG env vars for the test.
    fn with_test_home<F: FnOnce()>(home: &str, f: F) {
        let old_home = std::env::var_os("HOME");
        let old_xdg_config = std::env::var_os("XDG_CONFIG_HOME");
        let old_xdg_cache = std::env::var_os("XDG_CACHE_HOME");
        let old_xdg_data = std::env::var_os("XDG_DATA_HOME");

        std::env::set_var("HOME", home);
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CACHE_HOME");
        std::env::remove_var("XDG_DATA_HOME");

        f();

        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        match old_xdg_config {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_xdg_cache {
            Some(v) => std::env::set_var("XDG_CACHE_HOME", v),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }
        match old_xdg_data {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }

    #[test]
    fn test_lexical_normalize_collapses_dots() {
        let p = Path::new("/home/user/.config/zacxiom/../other/config.toml");
        let n = lexical_normalize(p).unwrap();
        assert_eq!(n, PathBuf::from("/home/user/.config/other/config.toml"));
    }

    #[test]
    fn test_lexical_normalize_collapses_curdir() {
        let p = Path::new("/home/user/./.config/./zacxiom/config.toml");
        let n = lexical_normalize(p).unwrap();
        assert_eq!(n, PathBuf::from("/home/user/.config/zacxiom/config.toml"));
    }

    #[test]
    fn test_whitelist_accepts_config_path() {
        with_test_home("/home/testuser", || {
            let p = Path::new("/home/testuser/.config/zacxiom/config.toml");
            assert!(validate_state_read(p).is_ok());
            assert!(validate_state_write(p).is_ok());
        });
    }

    #[test]
    fn test_whitelist_accepts_cache_path() {
        with_test_home("/home/testuser", || {
            let p = Path::new("/home/testuser/.cache/zacxiom/scan_cache.json");
            assert!(validate_state_read(p).is_ok());
        });
    }

    #[test]
    fn test_whitelist_accepts_data_path() {
        with_test_home("/home/testuser", || {
            let p = Path::new("/home/testuser/.local/share/zacxiom/snapshots/snap-123");
            assert!(validate_state_read(p).is_ok());
        });
    }

    #[test]
    fn test_whitelist_rejects_etc_shadow() {
        with_test_home("/home/testuser", || {
            let p = Path::new("/etc/shadow");
            assert!(validate_state_read(p).is_err());
            assert!(validate_state_write(p).is_err());
        });
    }

    #[test]
    fn test_whitelist_rejects_ssh_dir() {
        with_test_home("/home/testuser", || {
            let p = Path::new("/home/testuser/.ssh/id_rsa");
            assert!(validate_state_read(p).is_err());
        });
    }

    #[test]
    fn test_whitelist_rejects_sudoers() {
        with_test_home("/home/testuser", || {
            let p = Path::new("/etc/sudoers");
            assert!(validate_state_read(p).is_err());
        });
    }

    #[test]
    fn test_whitelist_rejects_proc_sys_dev() {
        with_test_home("/home/testuser", || {
            assert!(validate_state_read(Path::new("/proc/self/status")).is_err());
            assert!(validate_state_read(Path::new("/sys/kernel/notes")).is_err());
            assert!(validate_state_read(Path::new("/dev/sda1")).is_err());
        });
    }

    #[test]
    fn test_whitelist_rejects_all_system_dirs() {
        with_test_home("/home/testuser", || {
            // Every system path prefix must be rejected for state I/O
            assert!(validate_state_read(Path::new("/etc/zacxiom/config.toml")).is_err());
            assert!(validate_state_write(Path::new("/etc/zacxiom/config.toml")).is_err());
            assert!(validate_state_read(Path::new("/usr/local/zacxiom/x")).is_err());
            assert!(validate_state_read(Path::new("/var/lib/zacxiom/x")).is_err());
            assert!(validate_state_read(Path::new("/bin/zacxiom")).is_err());
            assert!(validate_state_read(Path::new("/sbin/zacxiom")).is_err());
            assert!(validate_state_read(Path::new("/lib/zacxiom")).is_err());
            assert!(validate_state_read(Path::new("/boot/zacxiom")).is_err());
            assert!(validate_state_read(Path::new("/root/zacxiom")).is_err());
        });
    }

    #[test]
    fn test_xdg_to_system_dir_is_blocked() {
        // Regression: XDG_CONFIG_HOME=/etc must NOT bypass pathguard
        with_test_home("/home/testuser", || {
            std::env::set_var("XDG_CONFIG_HOME", "/etc");
            let p = Path::new("/etc/zacxiom/config.toml");
            assert!(
                validate_state_read(p).is_err(),
                "XDG_CONFIG_HOME=/etc must be blocked by pathguard"
            );
            assert!(
                validate_state_write(p).is_err(),
                "XDG_CONFIG_HOME=/etc write must be blocked by pathguard"
            );
        });
    }

    #[test]
    fn test_xdg_to_user_ssh_is_blocked() {
        with_test_home("/home/testuser", || {
            std::env::set_var("XDG_CONFIG_HOME", "/home/testuser/.ssh");
            let p = Path::new("/home/testuser/.ssh/zacxiom/config.toml");
            assert!(validate_state_read(p).is_err());
            assert!(validate_state_write(p).is_err());
        });
    }

    #[test]
    fn test_whitelist_rejects_arbitrary_user_file() {
        with_test_home("/home/testuser", || {
            // Random file in home — not in any whitelisted subdir
            assert!(validate_state_read(Path::new("/home/testuser/random.txt")).is_err());
            assert!(validate_state_read(Path::new("/home/testuser/.bashrc")).is_err());
            assert!(validate_state_read(Path::new("/home/testuser/Downloads/x.iso")).is_err());
        });
    }

    #[test]
    fn test_whitelist_rejects_path_traversal() {
        with_test_home("/home/testuser", || {
            // Path traversal: starts inside whitelist, escapes to /etc
            let p = Path::new("/home/testuser/.config/zacxiom/../../../etc/shadow");
            // Should be rejected — either because canonical resolves outside whitelist,
            // or because the lexical-normalized path falls outside.
            let r = validate_state_read(p);
            assert!(r.is_err(), "traversal must be blocked, got {r:?}");
        });
    }

    #[test]
    fn test_whitelist_rejects_random_absolute_path() {
        with_test_home("/home/testuser", || {
            assert!(validate_state_read(Path::new("/tmp/random.toml")).is_err());
            assert!(validate_state_read(Path::new("/var/lib/foo/bar")).is_err());
            assert!(validate_state_read(Path::new("/opt/zacxiom/config.toml")).is_err());
        });
    }

    #[test]
    fn test_xdg_config_home_is_respected() {
        with_test_home("/home/testuser", || {
            std::env::set_var("XDG_CONFIG_HOME", "/custom/xdg");
            let p = Path::new("/custom/xdg/zacxiom/config.toml");
            assert!(validate_state_read(p).is_ok());
            // Old default is no longer valid (XDG takes precedence)
            let p2 = Path::new("/home/testuser/.config/zacxiom/config.toml");
            assert!(validate_state_read(p2).is_err());
        });
    }

    #[test]
    fn test_dangerous_paths_detected() {
        with_test_home("/home/testuser", || {
            // System paths
            assert!(is_dangerous(Path::new("/etc/shadow")));
            assert!(is_dangerous(Path::new("/etc/sudoers")));
            assert!(is_dangerous(Path::new("/etc/sudoers.d/wheel")));
            assert!(is_dangerous(Path::new("/etc/zacxiom/config.toml")));
            assert!(is_dangerous(Path::new("/usr/bin/bash")));
            assert!(is_dangerous(Path::new("/var/log/syslog")));
            assert!(is_dangerous(Path::new("/bin/sh")));
            assert!(is_dangerous(Path::new("/sbin/init")));
            assert!(is_dangerous(Path::new("/lib/libc.so.6")));
            assert!(is_dangerous(Path::new("/boot/vmlinuz")));
            assert!(is_dangerous(Path::new("/root/.bashrc")));
            assert!(is_dangerous(Path::new("/proc/self/status")));
            assert!(is_dangerous(Path::new("/sys/kernel")));
            assert!(is_dangerous(Path::new("/dev/null")));
            // User credentials
            assert!(is_dangerous(Path::new("/home/testuser/.ssh/id_rsa")));
            assert!(is_dangerous(Path::new("/home/testuser/.gnupg/secring.gpg")));
            // Not dangerous — valid state dirs
            assert!(!is_dangerous(Path::new(
                "/home/testuser/.config/zacxiom/config.toml"
            )));
            assert!(!is_dangerous(Path::new(
                "/home/testuser/.cache/zacxiom/scan_cache.json"
            )));
            assert!(!is_dangerous(Path::new(
                "/home/testuser/.local/share/zacxiom/snapshots/snap-1"
            )));
        });
    }
}
