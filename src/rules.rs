// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Immutable safety rules and core types for Zacxiom.
//!
//! These rules are NON-OVERRIDABLE. No plugin, no config, no flag can bypass them.
//! This module is the single source of truth for what Zacxiom may and may not do.

use serde::Serialize;
use std::path::Path;

/// Cache domain classification for a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum CacheDomain {
    Browser,
    System,
    BuildArtifact,
    PackageManager,
    Developer,
    UserData,
    Unknown,
}

impl CacheDomain {
    /// Human-readable display name for summaries.
    pub fn display_name(&self) -> &str {
        match self {
            CacheDomain::Browser => "Browser Cache",
            CacheDomain::System => "System Cache",
            CacheDomain::BuildArtifact => "Build Artifacts",
            CacheDomain::PackageManager => "Package Cache",
            CacheDomain::Developer => "Developer Cache",
            CacheDomain::UserData => "Application Cache",
            CacheDomain::Unknown => "Other",
        }
    }
}

impl std::fmt::Display for CacheDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheDomain::Browser => write!(f, "browser"),
            CacheDomain::System => write!(f, "system"),
            CacheDomain::BuildArtifact => write!(f, "build_artifact"),
            CacheDomain::PackageManager => write!(f, "package_manager"),
            CacheDomain::Developer => write!(f, "developer"),
            CacheDomain::UserData => write!(f, "user_data"),
            CacheDomain::Unknown => write!(f, "unknown"),
        }
    }
}

/// Ownership classification.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Ownership {
    Package { pkg_name: String },
    System,
    User { uid: u32 },
    Orphan,
}

impl std::fmt::Display for Ownership {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ownership::Package { pkg_name } => write!(f, "package({pkg_name})"),
            Ownership::System => write!(f, "system"),
            Ownership::User { uid } => write!(f, "user({uid})"),
            Ownership::Orphan => write!(f, "orphan"),
        }
    }
}

/// Decision for a file after risk scoring.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Decision {
    Safe,
    LowRisk,
    Moderate,
    HighRisk,
    Protected,
    /// v11: Active developer environment — currently in use, never clean.
    /// Risk: ★★★★★ Critical. "Never clean what the developer is actively using."
    ProtectedActiveEnvironment,
}

impl Decision {
    pub fn is_cleanable(&self, smart: bool, force: bool) -> bool {
        match self {
            Decision::Safe => true,
            Decision::LowRisk => smart || force,
            Decision::Moderate => force,
            // v13: HighRisk is NEVER auto-cleanable, even with --force.
            // These are config files, credentials, project source — user must `rm` manually.
            // Previous behavior allowed --force to delete HighRisk, which caused data loss.
            Decision::HighRisk => false,
            Decision::Protected => false,                  // never
            Decision::ProtectedActiveEnvironment => false, // never — active env
        }
    }
}

/// The universal pipeline type flowing through every module.
#[derive(Debug, Clone, Serialize)]
pub struct ClassifiedFile {
    pub path: String,
    pub size: u64,
    pub cache_domain: CacheDomain,
    pub ownership: Ownership,
    pub risk_score: f64,
    pub risk_reasons: Vec<String>,
    pub decision: Decision,
    /// Engine classification category (v6.3.1 bridge).
    #[serde(default)]
    pub engine_category: String,
    /// Engine confidence score 0-100 (v6.3.1 bridge).
    #[serde(default)]
    pub engine_confidence: u8,
}

/// H2: Protected paths — hard-coded, NEVER removable.
/// Any path starting with one of these prefixes is Decision::Protected.
pub const PROTECTED_PATHS: &[&str] = &[
    "/boot/",
    "/etc/",
    "/sys/",
    "/proc/",
    "/dev/",
    "/bin/",
    "/sbin/",
    "/lib/",
    "/lib64/",
    "/usr/bin/",
    "/usr/sbin/",
    "/usr/lib/",
    "/usr/lib64/",
    "/usr/include/",
    "/usr/share/",
    "/var/lib/dpkg/",
    "/var/lib/rpm/",
    "/var/lib/pacman/",
];

/// v13: File extensions that are NEVER cleanable — disk images + crypto keys.
/// These are protected regardless of location (even in /tmp or cache dirs).
/// Deleting a disk image = losing a VM or installable OS. Deleting a .pem = losing access.
pub const PROTECTED_EXTENSIONS: &[&str] = &[
    ".iso",
    ".vmdk",
    ".vdi",
    ".vhd",
    ".vhdx",
    ".qcow2",
    ".qcow",
    ".ova",
    ".ovf",
    ".img",
    ".raw",
    ".wim",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    ".keystore",
    ".jks",
    ".gpg",
    ".asc",
];

/// v13: Check if a file has a protected extension (disk image, crypto key, etc.)
pub fn has_protected_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let lower = ext.to_lowercase();
    PROTECTED_EXTENSIONS
        .iter()
        .any(|p| p.trim_start_matches('.') == lower)
}

/// v13: Check if a file matches a protected glob pattern (e.g. id_rsa).
pub fn matches_protected_pattern(path: &Path, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let path_str = path.to_string_lossy();
    for pat in patterns {
        if let Ok(glob) = globset::Glob::new(pat) {
            let compiled = glob.compile_matcher();
            if compiled.is_match(name) || compiled.is_match(path_str.as_ref()) {
                return true;
            }
        }
    }
    false
}

/// H2 extended: protected per-user paths (resolved at scan time for each home dir).
pub fn user_protected_suffixes() -> &'static [&'static str] {
    &[".ssh/", ".gnupg/"]
}

/// Check if a path is protected by H2.
/// v13: Canonicalizes the path first to prevent symlink traversal attacks
/// (e.g. /tmp/link -> /etc/passwd would previously bypass protection).
pub fn is_protected(path: &Path) -> bool {
    // Try canonical form first — resolves symlinks and .. components
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = canon.to_string_lossy();
    if PROTECTED_PATHS
        .iter()
        .any(|prefix| path_str.starts_with(prefix))
    {
        return true;
    }
    // Also check the raw path — handles cases where canonicalize fails
    // (broken symlink, non-existent path) but the raw path is clearly protected
    let raw_str = path.to_string_lossy();
    PROTECTED_PATHS
        .iter()
        .any(|prefix| raw_str.starts_with(prefix))
}

/// Check if a path under a home directory is protected.
pub fn is_user_protected(home: &Path, path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let home_str = home.to_string_lossy();
    user_protected_suffixes()
        .iter()
        .any(|suffix| path_str.starts_with(&format!("{home_str}/{suffix}")))
}

/// The immutable output format: file → reason → risk → decision
#[allow(dead_code)]
pub const OUTPUT_FORMAT: &str = "file → reason → risk → decision";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protected_paths() {
        assert!(is_protected(Path::new("/etc/passwd")));
        assert!(is_protected(Path::new("/boot/vmlinuz")));
        assert!(is_protected(Path::new("/usr/bin/bash")));
        assert!(!is_protected(Path::new("/home/user/.cache/test")));
        assert!(!is_protected(Path::new("/var/cache/apt/archives/test.deb")));
    }

    #[test]
    fn test_protected_extensions() {
        assert!(has_protected_extension(Path::new(
            "/home/user/Downloads/ubuntu.iso"
        )));
        assert!(has_protected_extension(Path::new("/tmp/vm.vmdk")));
        assert!(has_protected_extension(Path::new("/home/user/cert.pem")));
        assert!(has_protected_extension(Path::new("/home/user/key.PEM"))); // case-insensitive
        assert!(!has_protected_extension(Path::new("/home/user/file.txt")));
        assert!(!has_protected_extension(Path::new("/home/user/noext")));
    }

    #[test]
    fn test_protected_pattern_matching() {
        let patterns = vec!["id_rsa".to_string(), "id_ed25519".to_string()];
        assert!(matches_protected_pattern(
            Path::new("/home/user/.ssh/id_rsa"),
            &patterns
        ));
        assert!(matches_protected_pattern(
            Path::new("/home/user/.ssh/id_ed25519"),
            &patterns
        ));
        assert!(!matches_protected_pattern(
            Path::new("/home/user/.ssh/config"),
            &patterns
        ));
    }

    #[test]
    fn test_high_risk_never_cleanable_v13() {
        // v13: HighRisk must NEVER be cleanable, even with --force
        assert!(!Decision::HighRisk.is_cleanable(false, false));
        assert!(!Decision::HighRisk.is_cleanable(true, false));
        assert!(!Decision::HighRisk.is_cleanable(false, true));
        assert!(!Decision::HighRisk.is_cleanable(true, true));
    }

    #[test]
    fn test_user_protected() {
        let home = Path::new("/home/user");
        assert!(is_user_protected(home, Path::new("/home/user/.ssh/id_rsa")));
        assert!(is_user_protected(
            home,
            Path::new("/home/user/.gnupg/secret")
        ));
        assert!(!is_user_protected(
            home,
            Path::new("/home/user/.cache/mozilla")
        ));
    }

    #[test]
    fn test_decision_cleanable() {
        assert!(Decision::Safe.is_cleanable(false, false));
        assert!(!Decision::LowRisk.is_cleanable(false, false));
        assert!(Decision::LowRisk.is_cleanable(true, false));
        assert!(Decision::LowRisk.is_cleanable(false, true));
        assert!(!Decision::Moderate.is_cleanable(true, false));
        assert!(Decision::Moderate.is_cleanable(false, true));
        assert!(!Decision::Protected.is_cleanable(true, true));
    }

    #[test]
    fn test_high_risk_requires_force() {
        // v13: HighRisk is NEVER cleanable (was: cleanable with --force)
        assert!(!Decision::HighRisk.is_cleanable(false, false));
        assert!(!Decision::HighRisk.is_cleanable(true, false));
        assert!(!Decision::HighRisk.is_cleanable(false, true)); // changed: was true
        assert!(!Decision::HighRisk.is_cleanable(true, true)); // changed: was true
    }
}
