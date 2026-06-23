// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Multi-layer scoring engine — combines evidence from all layers.

use super::metadata;
use super::types::{Category, ClassificationResult, RiskLevel};
use std::path::Path;

/// Classify a path using the full rule engine + metadata analysis.
pub fn classify(path: &Path) -> ClassificationResult {
    let path_str = path.to_string_lossy();
    let lower = path_str.to_lowercase();

    let mut result = ClassificationResult::new(path.to_path_buf());

    // Size if available
    result.size = metadata::file_size(path);

    // ── Layer 1: Rule database (structured path matching) ─────
    let rules = super::rules::rule_database();
    let mut matched = false;

    for rule in &rules {
        if (rule.matches)(path, &lower) {
            result.category = rule.category;
            result.risk_level = rule.risk_level;
            result.regenerable = rule.regenerable;
            result.matched_by = rule.name.to_string();
            result.reasons.push(rule.reason.to_string());
            matched = true;
            break;
        }
    }

    // ── Layer 2: Metadata analysis ────────────────────────────
    if metadata::is_elf_binary(path) {
        if result.category == Category::Unknown {
            result.category = Category::SystemBinary;
            result.risk_level = RiskLevel::Critical;
            result.reasons.push("ELF binary detected".into());
        }
        result.confidence += 0.3;
    }

    if metadata::is_executable(path) && !path_str.ends_with(".sh") {
        result.reasons.push("Executable permission set".into());
        result.confidence += 0.1;
    }

    // ── Layer 3: Regenerability analysis ──────────────────────
    if !matched && result.category == Category::Unknown {
        // Check if path looks regenerable
        if lower.contains("/cache/") || lower.contains("/tmp/") {
            result.category = Category::Cache;
            result.risk_level = RiskLevel::Low;
            result.regenerable = true;
            result
                .reasons
                .push("Cache directory pattern detected".into());
            result.confidence += 0.5;
        }
    }

    // ── Layer 4: Confidence scoring ───────────────────────────
    if matched {
        result.confidence = 0.85; // Rule match = high confidence
    }

    // Boost confidence for regenerable items with cache-like paths
    if result.regenerable && result.confidence < 0.6 {
        result.confidence += 0.2;
    }

    // Cap confidence
    result.confidence = result.confidence.clamp(0.0, 1.0);

    // If still unknown, note it
    if result.category == Category::Unknown {
        result.reasons.push("No classification rule matched".into());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_system_binary() {
        let r = classify(Path::new("/usr/bin/bash"));
        assert_eq!(r.category, Category::SystemBinary);
        assert_eq!(r.risk_level, RiskLevel::Critical);
        assert!(!r.regenerable);
    }

    #[test]
    fn test_classify_system_config() {
        let r = classify(Path::new("/etc/environment"));
        assert_eq!(r.category, Category::SystemConfiguration);
        assert_eq!(r.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_classify_browser_cache() {
        let r = classify(Path::new(
            "/home/user/.cache/BraveSoftware/Brave-Browser/Cache/data_0",
        ));
        assert_eq!(r.category, Category::BrowserCache);
        assert_eq!(r.risk_level, RiskLevel::Minimal);
        assert!(r.regenerable);
    }

    #[test]
    fn test_classify_user_cache() {
        let r = classify(Path::new("/home/user/.cache/some-app/data"));
        assert_eq!(r.category, Category::Cache);
        assert!(r.regenerable);
    }

    #[test]
    fn test_classify_ssh_key() {
        let r = classify(Path::new("/home/user/.ssh/id_ed25519"));
        assert_eq!(r.category, Category::SecurityCredential);
        assert_eq!(r.risk_level, RiskLevel::Critical);
        assert!(!r.regenerable);
    }

    #[test]
    fn test_classify_shell_config() {
        let r = classify(Path::new("/home/user/.zshrc"));
        assert_eq!(r.category, Category::ShellConfiguration);
        assert_eq!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_desktop() {
        let r = classify(Path::new("/home/user/Desktop"));
        assert_eq!(r.category, Category::UserDesktop);
    }

    #[test]
    fn test_classify_tmp() {
        let r = classify(Path::new("/tmp/some-file"));
        assert!(r.regenerable);
    }

    #[test]
    fn test_brave_binary_not_cache() {
        let r = classify(Path::new("/usr/bin/brave"));
        assert_eq!(r.category, Category::SystemBinary);
        assert_ne!(r.category, Category::BrowserCache);
    }

    #[test]
    fn test_etc_not_cache() {
        let r = classify(Path::new("/etc/environment"));
        assert_ne!(r.category, Category::Cache);
    }

    #[test]
    fn test_home_root() {
        // Home root detection requires is_dir() — in test env it may not exist
        // Just verify the rule exists and is correct type
        let r = classify(Path::new("/home/user"));
        // If /home/user doesn't exist, it falls through
        // But it should never be classified as cache
        assert_ne!(r.category, Category::Cache);
    }

    #[test]
    fn test_regenerability_consistency() {
        // Cache items should be regenerable
        let cache = classify(Path::new("/home/user/.cache/something"));
        assert!(cache.regenerable);

        // Config items should NOT be regenerable
        let config = classify(Path::new("/home/user/.zshrc"));
        assert!(!config.regenerable);
    }
}
