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
            // v7: Propagate artifact intelligence fields
            result.created_by = rule.created_by.to_string();
            result.regenerated_by = rule.regenerated_by.to_string();
            result.depends_on = rule.depends_on.to_string();
            result.deletion_impact = rule.deletion_impact.to_string();
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
        assert_eq!(r.category, Category::ProjectAsset);
        assert_eq!(r.matched_by, "shell-script");

        let r2 = classify(Path::new("/home/user/project/build.sh"));
        assert_eq!(r2.category, Category::ProjectAsset);
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
        assert_eq!(r.matched_by, "rustup-any");

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

    // ═══════════════════════════════════════════════════════════
    // v6.4.0: Build Artifact & Policy Refinement tests
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_bare_target_is_build_cache() {
        // "target" without slashes must still be recognized
        let r = classify(Path::new("target"));
        assert_eq!(r.category, Category::BuildCache);
        assert_eq!(r.matched_by, "cache-build-target");
    }

    #[test]
    fn test_node_dist_is_build_cache() {
        let r1 = classify(Path::new("dist"));
        assert_eq!(r1.category, Category::BuildCache);

        let r2 = classify(Path::new("/home/user/project/dist/bundle.js"));
        assert_eq!(r2.category, Category::BuildCache);

        let r3 = classify(Path::new("/home/user/project/.next/static/chunks/app.js"));
        assert_eq!(r3.category, Category::BuildCache);
    }

    #[test]
    fn test_generic_build_output_is_build_cache() {
        let r1 = classify(Path::new("/home/user/project/build/app.o"));
        assert_eq!(r1.category, Category::BuildCache);

        let r2 = classify(Path::new("/home/user/project/out/production/app.jar"));
        assert_eq!(r2.category, Category::BuildCache);

        let r3 = classify(Path::new("/home/user/project/obj/main.obj"));
        assert_eq!(r3.category, Category::BuildCache);
    }

    #[test]
    fn test_toolchain_installation_is_not_unknown() {
        let r = classify(Path::new("/home/user/.rustup/toolchains/stable-x86_64"));
        assert_ne!(r.category, Category::Unknown);
        assert_eq!(r.category, Category::ToolchainInstallation);
    }

    // ═══════════════════════════════════════════════════════════
    // v6.4.0 deep audit: ZERO Rustup files may become cleanable
    // in safe mode. Every path inside ~/.rustup/ must be classified
    // as ToolchainInstallation or ToolchainManager — NEVER as a
    // cleanable category (Cache, BuildCache, ApplicationConfiguration, etc.)
    // ═══════════════════════════════════════════════════════════

    /// Helper: assert a path is classified as toolchain-related
    /// (ToolchainInstallation or ToolchainManager), never cleanable.
    fn assert_toolchain(path: &str) {
        let r = classify(Path::new(path));
        assert!(
            matches!(
                r.category,
                Category::ToolchainInstallation | Category::ToolchainManager
            ),
            "BUG: {} classified as {:?} (by {}) — expected ToolchainInstallation/ToolchainManager",
            path,
            r.category,
            r.matched_by,
        );
        // Toolchain categories must NOT be cleanable in safe mode
        assert!(
            !r.category.is_cleanable(),
            "BUG: {} classified as cleanable category {:?}",
            path,
            r.category,
        );
    }

    #[test]
    fn test_rustup_downloads_is_toolchain() {
        // ~/.rustup/downloads/ — active toolchain downloads
        assert_toolchain("/home/user/.rustup/downloads/stable-x86_64.tar.gz");
        assert_toolchain("/home/dev/.rustup/downloads/nightly-x86_64.partial");
    }

    #[test]
    fn test_rustup_tmp_is_toolchain() {
        // ~/.rustup/tmp/ — temporary extraction files
        assert_toolchain("/home/user/.rustup/tmp/rustup-temp-extract/lib/rustlib/src");
        assert_toolchain("/home/dev/.rustup/tmp/staging/manifest");
    }

    #[test]
    fn test_rustup_update_hashes_is_toolchain() {
        // ~/.rustup/update-hashes/ — must be ToolchainInstallation, not DownloadedArtifact
        assert_toolchain("/home/user/.rustup/update-hashes/stable-x86_64");
        assert_toolchain("/home/dev/.rustup/update-hashes/nightly-x86_64");
    }

    #[test]
    fn test_rustup_settings_toml_is_toolchain() {
        // ~/.rustup/settings.toml — rustup config, must NOT be ApplicationConfiguration
        assert_toolchain("/home/user/.rustup/settings.toml");
    }

    #[test]
    fn test_rustup_toolchain_bin_is_toolchain() {
        // Binaries inside toolchain — must be ToolchainInstallation, not SystemBinary
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/bin/rustc");
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/bin/cargo");
        assert_toolchain("/home/user/.rustup/toolchains/nightly-x86_64/bin/rustfmt");
    }

    #[test]
    fn test_rustup_toolchain_lib_is_toolchain() {
        // Libraries inside toolchain
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/core/src/lib.rs");
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/lib/libstd.so");
    }

    #[test]
    fn test_rustup_toolchain_manifest_is_toolchain() {
        // Manifest files inside toolchain — must NOT be BuildManifest
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/manifest-cargo");
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/manifest-rustc");
    }

    #[test]
    fn test_rustup_toolchain_cargo_toml_is_toolchain() {
        // Cargo.toml inside toolchain — must be ToolchainInstallation, not BuildManifest
        assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/core/Cargo.toml");
    }

    #[test]
    fn test_rustup_toolchain_shell_script_is_toolchain() {
        // .sh files inside toolchain — must be ToolchainInstallation, not ShellScript
        assert_toolchain(
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/etc/lldb_lookup.sh",
        );
    }

    #[test]
    fn test_rustup_comprehensive_all_subpaths() {
        // Exhaustive enumeration of every known ~/.rustup/ subdirectory structure.
        // If ANY of these is NOT ToolchainInstallation/ToolchainManager, that's a bug.
        let rustup_paths = [
            // Root
            "/home/user/.rustup",
            // Toolchains — the bulk of installed files
            "/home/user/.rustup/toolchains",
            "/home/user/.rustup/toolchains/stable-x86_64",
            "/home/user/.rustup/toolchains/stable-x86_64/bin",
            "/home/user/.rustup/toolchains/stable-x86_64/bin/rustc",
            "/home/user/.rustup/toolchains/stable-x86_64/bin/cargo",
            "/home/user/.rustup/toolchains/stable-x86_64/bin/rustfmt",
            "/home/user/.rustup/toolchains/stable-x86_64/bin/clippy-driver",
            "/home/user/.rustup/toolchains/stable-x86_64/lib",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/libstd-12345.so",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/core/src/lib.rs",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/core/Cargo.toml",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/manifest-cargo",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/manifest-rustc",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/manifest-std",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/etc/lldb_lookup.sh",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-12345.rlib",
            "/home/user/.rustup/toolchains/stable-x86_64/share",
            "/home/user/.rustup/toolchains/stable-x86_64/share/doc",
            "/home/user/.rustup/toolchains/stable-x86_64/share/man",
            // Downloads
            "/home/user/.rustup/downloads",
            "/home/user/.rustup/downloads/stable-x86_64.tar.gz",
            "/home/user/.rustup/downloads/nightly-x86_64.partial",
            // Temporary extraction
            "/home/user/.rustup/tmp",
            "/home/user/.rustup/tmp/rustup-temp-extract",
            "/home/user/.rustup/tmp/rustup-temp-extract/lib/rustlib/src",
            "/home/user/.rustup/tmp/staging/manifest",
            // Update hashes
            "/home/user/.rustup/update-hashes",
            "/home/user/.rustup/update-hashes/stable-x86_64",
            "/home/user/.rustup/update-hashes/nightly-x86_64",
            // Config
            "/home/user/.rustup/settings.toml",
            // Metadata
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/install.log",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/components",
            "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/rust-installer-version",
        ];

        for path in &rustup_paths {
            assert_toolchain(path);
        }
    }
}
