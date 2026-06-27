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
            CacheDomain::Developer => "Developer Tools",
            CacheDomain::UserData => "User Data",
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
}

impl Decision {
    pub fn is_cleanable(&self, smart: bool, force: bool) -> bool {
        match self {
            Decision::Safe => true,
            Decision::LowRisk => smart || force,
            Decision::Moderate => force,
            Decision::HighRisk => force, // v10: --force allows HighRisk after confirmation
            Decision::Protected => false, // never
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

/// H2 extended: protected per-user paths (resolved at scan time for each home dir).
pub fn user_protected_suffixes() -> &'static [&'static str] {
    &[".ssh/", ".gnupg/"]
}

/// Check if a path is protected by H2.
pub fn is_protected(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    PROTECTED_PATHS
        .iter()
        .any(|prefix| path_str.starts_with(prefix))
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
        // HighRisk must NOT be cleanable without --force
        assert!(!Decision::HighRisk.is_cleanable(false, false));
        assert!(!Decision::HighRisk.is_cleanable(true, false));
        // HighRisk IS cleanable with --force (after explicit confirmation)
        assert!(Decision::HighRisk.is_cleanable(false, true));
        assert!(Decision::HighRisk.is_cleanable(true, true));
    }
}
