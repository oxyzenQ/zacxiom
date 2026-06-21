//! Ownership detection — package vs user vs system vs orphan.
//!
//! v1 uses dpkg -S for package lookup (Debian/Ubuntu).
//! Falls back to path-based heuristics for non-dpkg systems.

use crate::rules::{is_protected, is_user_protected, Ownership};
use std::path::Path;
use std::process::Command;

/// Determine ownership of a file.
pub fn detect(path: &Path) -> Ownership {
    // Check dpkg ownership first (Debian/Ubuntu)
    if let Some(pkg) = dpkg_owns(path) {
        return Ownership::Package { pkg_name: pkg };
    }

    // Check if file is in a home directory
    if let Some(home) = std::env::var_os("HOME") {
        let home = Path::new(&home);
        if path.starts_with(home) {
            // Check user-protected paths (H2)
            if is_user_protected(home, path) {
                return Ownership::System;
            }
            // Use fallback uid 0 if we can't determine (non-unix or test env)
            let uid = get_uid();
            return Ownership::User { uid };
        }
    }

    // If it's a protected system path, mark as system
    if is_protected(path) {
        return Ownership::System;
    }

    // Anything else is orphan
    Ownership::Orphan
}

/// Get current user id in a cross-platform way.
#[cfg(unix)]
fn get_uid() -> u32 {
    unsafe { libc::getuid() }
}

#[cfg(not(unix))]
fn get_uid() -> u32 {
    0
}

/// Query dpkg for the package owning a file.
fn dpkg_owns(path: &Path) -> Option<String> {
    let output = Command::new("dpkg")
        .args(["-S", &path.to_string_lossy()])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // dpkg -S outputs "pkgname: /path/to/file"
        stdout.split(':').next().map(|s| s.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_home_file() {
        let home = std::env::var_os("HOME").unwrap();
        let test_path = Path::new(&home).join(".cache/test_file");
        let ownership = detect(&test_path);
        assert!(matches!(ownership, Ownership::User { .. }));
    }

    #[test]
    fn test_system_path_is_system() {
        let ownership = detect(Path::new("/etc/passwd"));
        assert!(matches!(ownership, Ownership::System));
    }

    #[test]
    fn test_orphan_non_home_non_system() {
        // /opt or /srv without package — should be orphan
        let ownership = detect(Path::new("/opt/some-app/cache/data.bin"));
        assert!(matches!(ownership, Ownership::Orphan));
    }
}
