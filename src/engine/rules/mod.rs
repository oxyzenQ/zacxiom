// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Structured rule database — replaces giant if/else chains.
//!
//! Rules are ordered by priority. First match wins.
//! Each rule specifies a path pattern, resulting category, and risk level.
//!
//! v7: Rules carry artifact intelligence — ownership, regeneration,
//! dependency, and deletion impact metadata.

mod database;

use super::types::{Category, RiskLevel};
use std::path::Path;
use std::sync::OnceLock;

/// A single classification rule.
///
/// v7: Enriched with artifact intelligence fields.
pub struct Rule {
    pub name: &'static str,
    /// Match logic: returns true if this rule applies to the given path.
    pub matches: fn(&Path, &str) -> bool,
    pub category: Category,
    pub risk_level: RiskLevel,
    pub regenerable: bool,
    pub reason: &'static str,
    // ── v7: Artifact Intelligence fields ──────────────────────
    /// Who created this artifact? (e.g. "Cargo", "Rustup", "npm", "Browser")
    pub created_by: &'static str,
    /// How to regenerate this artifact? (e.g. "cargo build", "rustup toolchain install")
    pub regenerated_by: &'static str,
    /// What does this artifact depend on? (e.g. "Cargo.toml", "package.json")
    pub depends_on: &'static str,
    /// What happens if this artifact is deleted?
    pub deletion_impact: &'static str,
}

/// Build the full rule database in priority order.
/// Cached via OnceLock — called once, shared across all classify() invocations.
/// Priority: system-protected > home-critical > config > cache > app-specific > fallback.
pub fn rule_database() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(database::build_rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::Category;
    use std::path::Path;

    fn check(path: &str, expected_category: Category) {
        let p = Path::new(path);
        let rules = rule_database();
        for rule in rules {
            let path_str = path.to_lowercase();
            if (rule.matches)(p, &path_str) {
                assert_eq!(
                    rule.category, expected_category,
                    "Path '{}' matched rule '{}' with category {:?}, expected {:?}",
                    path, rule.name, rule.category, expected_category
                );
                return;
            }
        }
        panic!(
            "No rule matched path '{}' (expected {:?})",
            path, expected_category
        );
    }

    #[test]
    fn test_system_paths_protected() {
        // System binaries under /usr
        check("/usr/bin", Category::SystemBinary);
        check("/usr/bin/ls", Category::SystemBinary);
        // System configuration
        check("/etc", Category::SystemConfiguration);
        // Virtual filesystems (with subpath to trigger match)
        check("/sys/kernel", Category::VirtualFilesystem);
        check("/proc/cpuinfo", Category::VirtualFilesystem);
        check("/dev/null", Category::VirtualFilesystem);
    }

    #[test]
    fn test_cargo_build_artifacts() {
        check(
            "/home/user/project/target/debug/deps/app-abc.rlib",
            Category::BuildCache,
        );
    }

    #[test]
    fn test_ssh_is_security_credential() {
        for p in &["/home/user/.ssh", "/home/user/.ssh/id_rsa"] {
            check(p, Category::SecurityCredential);
        }
    }

    #[test]
    fn test_config_dir_is_app_config() {
        check("/home/user/.config", Category::ApplicationConfiguration);
    }

    #[test]
    fn test_home_root_is_user_home() {
        check("/home/user", Category::UserHomeRoot);
    }
}
