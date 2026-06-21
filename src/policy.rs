// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! User-defined policy engine.
//!
//! Reads ~/.config/zacxiom/policy.json for user-defined safety rules.
//! User policies can ADD protection, never remove system H-rules.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Policy {
    /// Additional paths to protect (beyond H2).
    pub protected_paths: Vec<String>,
    /// Maximum file size for auto-clean (bytes, 0 = unlimited).
    pub max_file_size: u64,
    /// Domains to always skip.
    pub skip_domains: Vec<String>,
    /// Minimum risk score to auto-clean.
    pub min_risk_for_clean: f64,
}

#[allow(dead_code)]
impl Policy {
    pub fn load() -> Self {
        let path = policy_path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Policy::default()
        }
    }

    pub fn is_user_protected(&self, path: &str) -> bool {
        self.protected_paths
            .iter()
            .any(|p| path.starts_with(p.as_str()))
    }

    pub fn should_skip_domain(&self, domain: &str) -> bool {
        self.skip_domains
            .iter()
            .any(|d| d.eq_ignore_ascii_case(domain))
    }
}

fn policy_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".config/zacxiom/policy.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let p = Policy::default();
        assert!(p.protected_paths.is_empty());
        assert_eq!(p.max_file_size, 0);
    }

    #[test]
    fn test_user_protected() {
        let p = Policy {
            protected_paths: vec!["/home/user/projects/".into()],
            ..Default::default()
        };
        assert!(p.is_user_protected("/home/user/projects/myapp/target/debug"));
        assert!(!p.is_user_protected("/home/user/.cache/test"));
    }

    #[test]
    fn test_skip_domain() {
        let p = Policy {
            skip_domains: vec!["browser".into()],
            ..Default::default()
        };
        assert!(p.should_skip_domain("browser"));
        assert!(!p.should_skip_domain("system"));
    }
}
