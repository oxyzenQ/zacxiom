// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Multi-layer scoring engine — combines evidence from all layers.

use super::metadata;
use super::types::{Category, ClassificationResult, RiskLevel};
use std::path::Path;

/// Fast classification without confidence scoring (v6.3.1).
/// Returns (category_display_string, 0) — minimal allocation for scan pipeline.
pub fn classify_fast(path: &Path) -> (&'static str, u8) {
    let lower = path.to_string_lossy().to_lowercase();
    let rules = super::rules::rule_database();
    for rule in rules {
        if (rule.matches)(path, &lower) {
            return (rule.category.display(), 100);
        }
    }
    (Category::Unknown.display(), 0)
}

/// Classify a path using the full rule engine + metadata analysis.
pub fn classify(path: &Path) -> ClassificationResult {
    let path_str = path.to_string_lossy();
    let lower = path_str.to_lowercase();

    let mut result = ClassificationResult::new(path.to_path_buf());

    // Size if available
    result.size = metadata::file_size(path);

    // ── Layer 1: Rule database (structured path matching) ─────
    let rules = super::rules::rule_database(); // cached OnceLock
    let mut matched = false;

    for rule in rules {
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

    // ── Layer 2.5: Project/workspace detection (filesystem-aware) ──
    // Only when no rule matched and path is a directory.
    // This is expensive (filesystem access) but only called from explain,
    // not from the scan pipeline (which uses classify_fast).
    if !matched && path.is_dir() {
        if path.join("Cargo.toml").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-rust".to_string();
            result
                .reasons
                .push("Rust project workspace detected (Cargo.toml present)".into());
            matched = true;
        } else if path.join("package.json").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-node".to_string();
            result
                .reasons
                .push("Node.js project workspace detected (package.json present)".into());
            matched = true;
        } else if path.join("go.mod").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-go".to_string();
            result
                .reasons
                .push("Go project workspace detected (go.mod present)".into());
            matched = true;
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

    if metadata::is_regular_executable(path) && !path_str.ends_with(".sh") {
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

    // v6.3: Numerical confidence scoring
    super::confidence::score(&mut result, path, &lower);

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

    // ═══════════════════════════════════════════════════════════
    // v6.3.3: Semantic Explain Engine tests
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_rust_cargo_toml_is_build_manifest() {
        let r = classify(Path::new("Cargo.toml"));
        assert_eq!(r.category, Category::BuildManifest);
        assert_eq!(r.matched_by, "rust-cargo-toml");
        assert!(!r.regenerable);
    }

    #[test]
    fn test_rust_cargo_lock_is_dependency_lockfile() {
        let r = classify(Path::new("Cargo.lock"));
        assert_eq!(r.category, Category::DependencyLockfile);
        assert_eq!(r.matched_by, "rust-cargo-lock");
        assert!(r.regenerable);
    }

    #[test]
    fn test_cargo_toml_not_generic_config() {
        // Cargo.toml must NOT be classified as generic ApplicationConfiguration
        let r = classify(Path::new("/home/user/project/Cargo.toml"));
        assert_ne!(r.category, Category::ApplicationConfiguration);
        assert_eq!(r.category, Category::BuildManifest);
    }

    #[test]
    fn test_node_package_json_is_build_manifest() {
        let r = classify(Path::new("package.json"));
        assert_eq!(r.category, Category::BuildManifest);
        assert_eq!(r.matched_by, "node-package-json");
    }

    #[test]
    fn test_go_mod_is_build_manifest() {
        let r = classify(Path::new("go.mod"));
        assert_eq!(r.category, Category::BuildManifest);
        assert_eq!(r.matched_by, "go-mod");
    }

    #[test]
    fn test_source_dir_classification() {
        let r = classify(Path::new("src"));
        assert_eq!(r.category, Category::SourceDirectory);
        assert_eq!(r.matched_by, "source-dir");

        let r2 = classify(Path::new("/home/user/project/src"));
        assert_eq!(r2.category, Category::SourceDirectory);

        let r3 = classify(Path::new("/home/user/project/src/main.rs"));
        assert_eq!(r3.category, Category::SourceDirectory);
    }

    #[test]
    fn test_usr_src_not_source_dir() {
        // /usr/src should NOT be SourceDirectory (system territory)
        let r = classify(Path::new("/usr/src/linux/foo.c"));
        // Should be matched by sys-lib or similar, NOT source-dir
        assert_ne!(r.matched_by, "source-dir");
    }

    #[test]
    fn test_shell_script_classification() {
        let r = classify(Path::new("scripts/install.sh"));
        assert_eq!(r.category, Category::ShellScript);
        assert_eq!(r.matched_by, "shell-script");

        let r2 = classify(Path::new("/home/user/project/build.sh"));
        assert_eq!(r2.category, Category::ShellScript);
    }

    #[test]
    fn test_rustup_home_is_toolchain_manager() {
        let r = classify(Path::new("/home/user/.rustup"));
        assert_eq!(r.category, Category::ToolchainManager);
        assert_eq!(r.matched_by, "rustup-home");
    }

    #[test]
    fn test_rustup_toolchains_is_toolchain_installation() {
        let r = classify(Path::new("/home/user/.rustup/toolchains"));
        assert_eq!(r.category, Category::ToolchainInstallation);
        assert_eq!(r.matched_by, "rustup-toolchains-dir");

        let r2 = classify(Path::new(
            "/home/user/.rustup/toolchains/stable-x86_64/bin/rustc",
        ));
        assert_eq!(r2.category, Category::ToolchainInstallation);
    }

    #[test]
    fn test_rustup_not_unknown() {
        // ~/.rustup and ~/.rustup/toolchains must never be Unknown
        let r1 = classify(Path::new("/home/dev/.rustup"));
        assert_ne!(r1.category, Category::Unknown);

        let r2 = classify(Path::new("/home/dev/.rustup/toolchains"));
        assert_ne!(r2.category, Category::Unknown);
    }

    #[test]
    fn test_build_manifest_not_unknown() {
        // Cargo.toml, package.json, go.mod must never be Unknown or generic config
        let paths = ["Cargo.toml", "package.json", "go.mod", "Cargo.lock"];
        for p in &paths {
            let r = classify(Path::new(p));
            assert_ne!(r.category, Category::Unknown, "{} was Unknown", p);
        }
    }

    #[test]
    fn test_target_dir_is_build_cache() {
        // Wider /target/ matching — not just debug/release
        let r1 = classify(Path::new("/home/user/project/target/doc/index.html"));
        assert_eq!(r1.category, Category::BuildCache);

        let r2 = classify(Path::new(
            "/home/user/project/target/wasm32-unknown-unknown/release/deps/app.wasm",
        ));
        assert_eq!(r2.category, Category::BuildCache);
    }
}
