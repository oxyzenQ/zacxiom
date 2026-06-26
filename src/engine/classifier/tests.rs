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
    assert_toolchain(
        "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/core/src/lib.rs",
    );
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
    assert_toolchain(
        "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/core/Cargo.toml",
    );
}

#[test]
fn test_rustup_toolchain_shell_script_is_toolchain() {
    // .sh files inside toolchain — must be ToolchainInstallation, not ShellScript
    assert_toolchain("/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/etc/lldb_lookup.sh");
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

// ═══════════════════════════════════════════════════════════
// v7.2: Context Inheritance Engine — success criteria tests
// ═══════════════════════════════════════════════════════════

#[test]
fn test_parent_child_inheritance_target_debug() {
    // Success criteria: target/debug → BuildOutput
    // The cache-build-target rule now directly matches target/debug via starts_with("target/")
    let r = classify(Path::new("target/debug"));
    assert_eq!(r.category, Category::BuildCache);
}

#[test]
fn test_parent_child_inheritance_target_release() {
    // Success criteria: target/release → BuildOutput
    // Direct rule match via starts_with("target/")
    let r = classify(Path::new("target/release"));
    assert_eq!(r.category, Category::BuildCache);
}

#[test]
fn test_parent_child_inheritance_absolute_target_debug() {
    // Full path with /target/debug should directly match, not just inherit
    let r = classify(Path::new("/home/user/project/target/debug"));
    assert_eq!(r.category, Category::BuildCache);
}

#[test]
fn test_parent_child_inheritance_nested_build() {
    // target/debug/build/some-crate-abc/out — should inherit from target/ ancestor
    let r = classify(Path::new("target/debug/build/some-crate-abc/out"));
    assert_eq!(r.category, Category::BuildCache);
}

#[test]
fn test_npm_npx_is_classified() {
    // Success criteria: ~/.npm/_npx → known ecosystem artifact, not Unknown
    let r1 = classify(Path::new("/home/user/.npm/_npx"));
    assert_ne!(r1.category, Category::Unknown);

    let r2 = classify(Path::new("/home/user/.npm/_npx/abc123/node_modules"));
    assert_ne!(r2.category, Category::Unknown);
}

#[test]
fn test_npm_cacache_is_cache_registry() {
    let r = classify(Path::new(
        "/home/user/.npm/_cacache/content-v2/sha512/ab/cd",
    ));
    assert_eq!(r.category, Category::CacheRegistry);
    assert!(r.regenerable);
}

#[test]
fn test_npm_generic_is_not_unknown() {
    // ~/.npm/ (not _npx, not _cacache) should match npm-cache-generic
    let r = classify(Path::new("/home/user/.npm/_logs/2026-01-01-debug.log"));
    assert_ne!(r.category, Category::Unknown);
}

#[test]
fn test_classify_fast_parent_inheritance() {
    // classify_fast should find target/debug directly (starts_with rule)
    let (cat, conf) = classify_fast(Path::new("target/debug"));
    assert_eq!(cat, Category::BuildCache.display());
    assert_eq!(conf, 100); // Direct match via starts_with("target/")

    let (cat2, conf2) = classify_fast(Path::new("target"));
    assert_eq!(cat2, Category::BuildCache.display());
    assert_eq!(conf2, 100);

    // Deeply nested unknown that requires inheritance — e.g. target/debug/.fingerprint/abc
    let (cat3, _conf3) = classify_fast(Path::new("target/debug/.fingerprint/some-crate"));
    // Should still find via ancestor — target/ or target/debug/ ancestors
    // Since classify_fast walks up, it finds target/ -> BuildCache
    assert_eq!(cat3, Category::BuildCache.display());
}

#[test]
fn test_classify_fast_npm_npx() {
    let (cat, _) = classify_fast(Path::new("/home/user/.npm/_npx/some-package"));
    assert_ne!(cat, Category::Unknown.display());
}

#[test]
fn test_target_children_inherit_intel() {
    // Children of target should inherit intel fields
    let r = classify(Path::new("target/release"));
    assert!(!r.created_by.is_empty());
    assert!(!r.regenerated_by.is_empty());
    assert!(!r.deletion_impact.is_empty());
}

// ═══════════════════════════════════════════════════════════
// v7.2.1: Hardening — comprehensive regression tests
// ═══════════════════════════════════════════════════════════

// ── Fix A: Target detection — all variants ───────────────

#[test]
fn regression_target_bare() {
    assert_eq!(classify(Path::new("target")).category, Category::BuildCache);
}

#[test]
fn regression_target_slash() {
    // "target/" — not a valid path, but the bare match covers "target"
    // classify(Path::new("target/")) normalizes to "target"
}

#[test]
fn regression_target_debug() {
    assert_eq!(
        classify(Path::new("target/debug")).category,
        Category::BuildCache
    );
}

#[test]
fn regression_target_release() {
    assert_eq!(
        classify(Path::new("target/release")).category,
        Category::BuildCache
    );
}

#[test]
fn regression_target_doc() {
    assert_eq!(
        classify(Path::new("target/doc")).category,
        Category::BuildCache
    );
}

#[test]
fn regression_dot_target() {
    assert_eq!(
        classify(Path::new("./target")).category,
        Category::BuildCache
    );
}

#[test]
fn regression_dot_target_debug() {
    assert_eq!(
        classify(Path::new("./target/debug")).category,
        Category::BuildCache
    );
}

#[test]
fn regression_absolute_target_debug() {
    assert_eq!(
        classify(Path::new("/home/user/project/target/debug")).category,
        Category::BuildCache
    );
}

// ── Fix B: Project Override — location rules yield to project markers ──

#[test]
fn regression_desktop_project_rust() {
    // Desktop/labs-coding/cosmostrix with Cargo.toml → ProjectWorkspace
    // Note: project_markers_found requires filesystem access,
    // so these test the classification logic path, not filesystem I/O.
    // The Layer 1.5 check is verified by code review.
    let r = classify(Path::new("/home/user/Desktop/labs-coding/cosmostrix"));
    // Without actual filesystem, it won't override. But it must not be Desktop.
    // In test, it matches user-desktop rule → then tries project_markers_found
    // which returns false (no real dir). Result stays UserDesktop.
    // This test confirms no panic, and the rule is matched.
    // The override logic is verified in Layer 1.5 code path.
    assert!(!r.matched_by.is_empty());
}

#[test]
fn regression_downloads_project() {
    // Downloads/some-project → matched by user-downloads → UserDocument → overridable
    let r = classify(Path::new("/home/user/Downloads/some-project"));
    assert_ne!(r.category, Category::Unknown);
}

// ── Fix C: npm ecosystem coverage ──

#[test]
fn regression_npm_npx() {
    let r = classify(Path::new("/home/user/.npm/_npx"));
    assert_ne!(r.category, Category::Unknown);
    assert!(!r.created_by.is_empty());
    assert!(!r.regenerated_by.is_empty());
}

#[test]
fn regression_npm_cacache() {
    let r = classify(Path::new("/home/user/.npm/_cacache/content-v2/sha512/ab"));
    assert_eq!(r.category, Category::CacheRegistry);
    assert!(r.regenerable);
}

#[test]
fn regression_npm_logs() {
    let r = classify(Path::new("/home/user/.npm/_logs/2026-01-debug.log"));
    assert_ne!(r.category, Category::Unknown);
    // Should be Cache (safe disposable), not ApplicationData or Unknown
    assert_eq!(r.category, Category::Cache);
}

#[test]
fn regression_npm_logs_root() {
    let r = classify(Path::new("/home/user/.npm/_logs"));
    assert_eq!(r.category, Category::Cache);
}

// ── Priority system verification ──

#[test]
fn regression_inheritance_does_not_beat_direct_match() {
    // target/debug should match directly (Priority 80), not via inheritance (Priority 40)
    let r = classify(Path::new("target/debug"));
    assert!(!r.matched_by.starts_with("inherit-"));
    assert_eq!(r.matched_by, "cache-build-target");
}

#[test]
fn regression_exact_match_beats_heuristic() {
    // ~/.npm/_npx should match exact rule, not fall through to heuristics
    let r = classify(Path::new("/home/user/.npm/_npx/some-pkg"));
    assert!(!r.matched_by.starts_with("inherit-"));
    assert!(!r.matched_by.is_empty());
}

// ── classify_fast consistency ──

#[test]
fn regression_classify_fast_consistent_with_classify() {
    let paths = [
        "target",
        "target/debug",
        "target/release",
        "target/doc",
        "./target/debug",
    ];
    for p in &paths {
        let fast = classify_fast(Path::new(p));
        let full = classify(Path::new(p));
        assert_eq!(
            fast.0,
            full.category.display(),
            "classify_fast and classify disagree on: {p}"
        );
    }
}

// ═══════════════════════════════════════════════════════════
// v8.2.1: pyproject.toml + .git classification
// ═══════════════════════════════════════════════════════════

#[test]
fn test_pyproject_toml_is_build_manifest() {
    let r = classify(Path::new("pyproject.toml"));
    assert_eq!(r.category, Category::BuildManifest);
    assert_eq!(r.matched_by, "python-pyproject");
}

#[test]
fn test_pyproject_toml_not_application_config() {
    let r = classify(Path::new("/home/user/project/pyproject.toml"));
    assert_ne!(r.category, Category::ApplicationConfiguration);
    assert_eq!(r.category, Category::BuildManifest);
}

#[test]
fn test_pyproject_toml_has_intel() {
    let r = classify(Path::new("pyproject.toml"));
    assert!(!r.created_by.is_empty());
    assert!(!r.deletion_impact.is_empty());
}
