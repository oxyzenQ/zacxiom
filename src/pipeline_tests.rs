use crate::confidence;
use crate::rules::{ClassifiedFile, Decision, Ownership};
use std::path::PathBuf;

/// Build a ClassifiedFile as if it went through the full scan pipeline
/// for a given path, then check the final decision and tier.
fn classify_via_pipeline(path: &str) -> (Decision, confidence::Tier, String) {
    let path_buf = PathBuf::from(path);

    // Step 1: Legacy cache classifier
    let domain = crate::cache::classify(&path_buf);

    // Step 2: Risk scoring — simplified but domain-aware.
    // Matches the logic in risk.rs domain_regenerability():
    //   Browser/BuildArtifact/PackageManager/Developer → Fully → Safe
    //   System → Partially → LowRisk (roughly)
    //   UserData/Unknown → NotRegenerable → Moderate (roughly)
    let (mut decision, risk_score) = match domain {
        crate::rules::CacheDomain::Browser
        | crate::rules::CacheDomain::BuildArtifact
        | crate::rules::CacheDomain::PackageManager
        | crate::rules::CacheDomain::Developer => (Decision::Safe, 0.0),
        crate::rules::CacheDomain::System => (Decision::LowRisk, 0.15),
        crate::rules::CacheDomain::UserData => (Decision::Moderate, 0.3),
        crate::rules::CacheDomain::Unknown => (Decision::Moderate, 0.3),
    };
    let mut risk_reasons: Vec<String> = vec![];

    // Step 3: Engine fast classify
    let eng = crate::engine::classify_fast(&path_buf);
    let engine_category = eng.0.to_string();
    let engine_confidence = eng.1;

    // Step 4: Bridge override (same logic as main.rs classify())
    // v7: Toolchain, installed software, dependency source, and downloaded
    // artifacts all require --smart — not auto-cleanable in safe mode.
    if decision == Decision::Safe {
        if engine_category == "Toolchain Installation"
            || engine_category == "Toolchain Manager"
            || engine_category == "Installed Software"
            || engine_category == "Dependency Source"
        {
            decision = Decision::LowRisk;
            risk_reasons.push(
                "Not disposable cache — regenerable but expensive to restore, requires --smart"
                    .into(),
            );
        } else if engine_category.contains("Downloaded") {
            decision = Decision::LowRisk;
            risk_reasons.push("Downloaded artifact: regenerable but expensive to restore".into());
        }
    }

    // Step 4b: v13 Engine-protected category enforcement.
    // If engine classifies as protected category, override to Protected.
    if matches!(
        engine_category.as_str(),
        "System Binary"
            | "System Configuration"
            | "System Data"
            | "Virtual Filesystem"
            | "Security Credential"
            | "Project Workspace"
            | "Source Code Directory"
            | "Package Manifest"
            | "Project Asset"
            | "Installed Software"
    ) {
        decision = Decision::Protected;
    }

    // Step 4c: v13 Extension protection — disk images, crypto keys, etc.
    // Must match pipeline::classify() behavior: override ANY decision to Protected.
    let path_obj = std::path::Path::new(path);
    if crate::rules::has_protected_extension(path_obj) {
        decision = Decision::Protected;
        risk_reasons
            .push("Protected file extension (disk image / crypto key) — never cleanable".into());
    }

    // Step 5: Build ClassifiedFile
    let cf = ClassifiedFile {
        path: path.to_string(),
        size: 1000,
        cache_domain: domain,
        ownership: Ownership::User { uid: 1000 },
        risk_score,
        risk_reasons,
        decision: decision.clone(),
        engine_category,
        engine_confidence,
    };

    // Step 6: Confidence tier
    let tier = confidence::confidence(&cf);

    (decision, tier, cf.engine_category.clone())
}

#[test]
fn test_toolchain_not_cleanable_in_safe_mode() {
    let (decision, tier, engine_cat) =
        classify_via_pipeline("/home/user/.rustup/toolchains/stable-x86_64/bin/rustc");

    // Engine must classify as Toolchain Installation
    assert_eq!(
        engine_cat, "Toolchain Installation",
        "Expected Toolchain Installation, got {}",
        engine_cat
    );

    // Decision must be LowRisk (not Safe)
    assert_eq!(decision, Decision::LowRisk);

    // Tier must NOT be ★★★★★ Maximum
    assert_ne!(tier, confidence::Tier::Maximum);

    // Tier should be ★★★★ High
    assert_eq!(tier, confidence::Tier::High);

    // Must NOT be cleanable in safe mode
    assert!(!decision.is_cleanable(false, false));

    // Must be cleanable in smart mode
    assert!(decision.is_cleanable(true, false));
}

#[test]
fn test_toolchain_src_file_not_cleanable_in_safe_mode() {
    // A .rs file inside rustup toolchains — must not be misclassified
    let (decision, _tier, engine_cat) = classify_via_pipeline(
        "/home/user/.rustup/toolchains/stable-x86_64/lib/rustlib/src/rust/library/std/src/lib.rs",
    );

    // Must NOT be classified as Source Code Directory
    assert_ne!(
        engine_cat, "Source Code Directory",
        "Rustup source file should not be Source Code Directory, got {}",
        engine_cat
    );

    // Must be Toolchain Installation
    assert_eq!(engine_cat, "Toolchain Installation");

    // Must NOT be cleanable in safe mode
    assert!(!decision.is_cleanable(false, false));
}

#[test]
fn test_rustup_home_not_cleanable_in_safe_mode() {
    let (decision, _tier, engine_cat) = classify_via_pipeline("/home/user/.rustup");

    assert_eq!(engine_cat, "Toolchain Manager");
    assert_eq!(decision, Decision::LowRisk);
    assert!(!decision.is_cleanable(false, false));
    assert!(decision.is_cleanable(true, false));
}

#[test]
fn test_regular_build_cache_still_cleanable() {
    // Regular build cache should still be cleanable in safe mode
    let (decision, _tier, engine_cat) =
        classify_via_pipeline("/home/user/project/target/debug/deps/app-abc.rlib");

    assert_eq!(engine_cat, "Build Output");
    assert_eq!(decision, Decision::Safe);
    assert!(decision.is_cleanable(false, false));
}

/// v6.4.0: Exhaustive audit — zero rustup files must be cleanable in safe mode.
/// Tests EVERY possible rustup sub-path to find policy leaks.
#[test]
fn audit_zero_rustup_leaks_in_safe_mode() {
    let rustup_paths = vec![
            // Root
            "/home/user/.rustup",
            // Toolchains
            "/home/user/.rustup/toolchains",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/libstd-123.rlib",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/lib.rs",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/index.html",
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/man/man1/rustc.1",
            // Update hashes
            "/home/user/.rustup/update-hashes/stable-x86_64-unknown-linux-gnu",
            "/home/user/.rustup/update-hashes/stable-x86_64-unknown-linux-gnu.sha256",
            // Downloads
            "/home/user/.rustup/downloads/stable-x86_64-unknown-linux-gnu.tar.gz",
            "/home/user/.rustup/downloads/stable-x86_64-unknown-linux-gnu.tar.xz",
            "/home/user/.rustup/downloads/RUSTUP_UPDATE_ROOT",
            // TMP
            "/home/user/.rustup/tmp/rustup-init-12345.tmp",
            "/home/user/.rustup/tmp/download-partial.gz",
            // Settings / metadata
            "/home/user/.rustup/settings.toml",
            "/home/user/.rustup/rustup-version",
            "/home/user/.rustup/version-file",
            // Misc
            "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/etc/lldb_commands",
        ];

    let mut leaks: Vec<String> = Vec::new();

    for path in &rustup_paths {
        let (decision, tier, engine_cat) = classify_via_pipeline(path);

        let is_cleanable_safe = decision.is_cleanable(false, false);
        let is_max_tier = tier == confidence::Tier::Maximum;

        if is_cleanable_safe || is_max_tier {
            leaks.push(format!(
                "{} → decision={:?} tier={:?} engine={}",
                path, decision, tier, engine_cat
            ));
        }
    }

    if !leaks.is_empty() {
        eprintln!("\n━━━ RUSTUP POLICY LEAKS ━━━");
        for leak in &leaks {
            eprintln!("  {}", leak);
        }
        eprintln!("━━━ {} LEAKS ━━━\n", leaks.len());
        panic!(
            "{} rustup paths still leak as cleanable/tier-max in safe mode",
            leaks.len()
        );
    }
}

// ═══════════════════════════════════════════════════════════
// v6.4.0: node_modules & cargo registry policy audit
// These are regenerable but expensive to restore — should
// require --smart, NOT be auto-cleanable in safe mode.
// ═══════════════════════════════════════════════════════════

#[test]
fn test_node_modules_not_cleanable_in_safe_mode() {
    // node_modules should be DownloadedArtifact → bridge → LowRisk
    let (decision, _tier, engine_cat) =
        classify_via_pipeline("/home/user/project/node_modules/lodash/index.js");

    assert_eq!(engine_cat, "Downloaded Artifact");
    assert_eq!(decision, Decision::LowRisk);
    assert!(
        !decision.is_cleanable(false, false),
        "node_modules should NOT be cleanable in safe mode"
    );
    assert!(
        decision.is_cleanable(true, false),
        "node_modules should be cleanable with --smart"
    );
}

#[test]
fn test_cargo_registry_not_cleanable_in_safe_mode() {
    // Cargo registry should be DownloadedArtifact → bridge → LowRisk
    let (decision, _tier, engine_cat) =
        classify_via_pipeline("/home/user/.cargo/registry/cache/index.crates.io-abc/syn-1.0.crate");

    assert_eq!(engine_cat, "Downloaded Artifact");
    assert_eq!(decision, Decision::LowRisk);
    assert!(
        !decision.is_cleanable(false, false),
        "cargo registry should NOT be cleanable in safe mode"
    );
    assert!(
        decision.is_cleanable(true, false),
        "cargo registry should be cleanable with --smart"
    );
}

#[test]
fn test_browser_cache_still_cleanable_in_safe_mode() {
    // Browser cache should remain ★★★★★ auto-cleanable
    let (decision, _tier, engine_cat) =
        classify_via_pipeline("/home/user/.cache/mozilla/firefox/profile/cache2/entry");

    assert_eq!(engine_cat, "Browser Cache");
    assert_eq!(decision, Decision::Safe);
    assert!(decision.is_cleanable(false, false)); // still auto-cleanable
}

#[test]
fn test_build_target_still_cleanable_in_safe_mode() {
    // Rust target/ directory should remain ★★★★★ auto-cleanable
    let (decision, _tier, engine_cat) =
        classify_via_pipeline("/home/user/project/target/debug/deps/app-abc.rlib");

    assert_eq!(engine_cat, "Build Output");
    assert_eq!(decision, Decision::Safe);
    assert!(decision.is_cleanable(false, false)); // still auto-cleanable
}

#[test]
fn test_downloads_not_cleanable_in_safe_mode() {
    // v13: ~/Downloads/installer.iso is now PROTECTED (disk image extension)
    // Previously: UserDocument → Moderate → cleanable with --force (caused data loss!)
    // Now: Protected extension override → NEVER cleanable, even with --force.
    let (decision, _tier, _engine_cat) =
        classify_via_pipeline("/home/user/Downloads/installer.iso");

    // ISO files must be Protected — never cleanable under any flag
    assert_eq!(
        decision,
        Decision::Protected,
        "ISO files must be Protected (v13 safety policy), got {:?}",
        decision
    );
    assert!(!decision.is_cleanable(false, false)); // NOT cleanable in safe mode
    assert!(!decision.is_cleanable(true, false)); // NOT cleanable with --smart
    assert!(!decision.is_cleanable(false, true)); // NOT cleanable with --force
    assert!(!decision.is_cleanable(true, true)); // NOT cleanable with --smart + --force
}

// ═══════════════════════════════════════════════════════════
// v6.4.0 ROOT-CAUSE AUDIT: Trace every path that could appear
// in "Node.js Modules" or "Cargo Registry & Build Cache" domains
// through the full pipeline. Identify which paths leak into the
// safe-mode cleanable set and WHY.
// ═══════════════════════════════════════════════════════════

/// Full pipeline trace for a single path — returns every intermediate result.
fn trace_pipeline(path: &str) -> PipelineTrace {
    let path_buf = PathBuf::from(path);

    // Step 1: Legacy cache classifier
    let cache_domain = crate::cache::classify(&path_buf);
    let cache_domain_str = format!("{:?}", cache_domain);

    // Step 2: Risk scoring (domain-aware)
    let (initial_decision, risk_score) = match cache_domain {
        crate::rules::CacheDomain::Browser
        | crate::rules::CacheDomain::BuildArtifact
        | crate::rules::CacheDomain::PackageManager
        | crate::rules::CacheDomain::Developer => (Decision::Safe, 0.0),
        crate::rules::CacheDomain::System => (Decision::LowRisk, 0.15),
        crate::rules::CacheDomain::UserData => (Decision::Moderate, 0.3),
        crate::rules::CacheDomain::Unknown => (Decision::Moderate, 0.3),
    };

    // Step 3: Engine fast classify
    let eng = crate::engine::classify_fast(&path_buf);
    let engine_category = eng.0.to_string();

    // Step 4: Find which rule matched
    let lower = path.to_lowercase();
    let rules = crate::engine::rules::rule_database();
    let matched_rule = rules
        .iter()
        .find(|r| (r.matches)(&path_buf, &lower))
        .map(|r| r.name.to_string())
        .unwrap_or_else(|| "NO_MATCH".to_string());

    // Step 5: Bridge override
    let mut final_decision = initial_decision.clone();
    if final_decision == Decision::Safe
        && (engine_category == "Toolchain Installation"
            || engine_category == "Toolchain Manager"
            || engine_category.contains("Downloaded"))
    {
        final_decision = Decision::LowRisk;
    }

    // Step 5a: v13 Engine-protected category enforcement
    if matches!(
        engine_category.as_str(),
        "System Binary"
            | "System Configuration"
            | "System Data"
            | "Virtual Filesystem"
            | "Security Credential"
            | "Project Workspace"
            | "Source Code Directory"
            | "Package Manifest"
            | "Project Asset"
            | "Installed Software"
    ) {
        final_decision = Decision::Protected;
    }

    // Step 5b: v13 Extension protection
    let path_obj = std::path::Path::new(path);
    if crate::rules::has_protected_extension(path_obj) {
        final_decision = Decision::Protected;
    }

    // Step 6: Confidence tier
    let cf = ClassifiedFile {
        path: path.to_string(),
        size: 1000,
        cache_domain,
        ownership: Ownership::User { uid: 1000 },
        risk_score,
        risk_reasons: vec![],
        decision: final_decision.clone(),
        engine_category: engine_category.clone(),
        engine_confidence: eng.1,
    };
    let tier = confidence::confidence(&cf);

    // Step 7: Domain key (what domain summary would show)
    let domain_key = crate::domain::summarize(&[cf])
        .first()
        .map(|d| d.domain.clone())
        .unwrap_or_default();

    PipelineTrace {
        path: path.to_string(),
        matched_rule,
        engine_category,
        cache_domain: cache_domain_str,
        initial_decision: format!("{:?}", initial_decision),
        final_decision: format!("{:?}", final_decision),
        tier: format!("{:?}", tier),
        cleanable_safe: final_decision.is_cleanable(false, false),
        cleanable_smart: final_decision.is_cleanable(true, false),
        domain_key,
    }
}

struct PipelineTrace {
    path: String,
    matched_rule: String,
    engine_category: String,
    cache_domain: String,
    initial_decision: String,
    final_decision: String,
    tier: String,
    cleanable_safe: bool,
    cleanable_smart: bool,
    domain_key: String,
}

#[test]
fn audit_node_modules_pipeline_trace() {
    let paths = vec![
        // Project node_modules
        "/home/user/project/node_modules/lodash/index.js",
        "/home/user/project/node_modules/react/package.json",
        "/home/user/project/node_modules/@types/node/index.d.ts",
        "/home/user/project/node_modules/.package-lock.json",
        "/home/user/project/node_modules/esbuild/bin/esbuild",
        // The node_modules directory ITSELF (no trailing slash)
        "/home/user/project/node_modules",
        // NPX cache
        "/home/user/.npm/_npx/d3b97f1234/node_modules/@zed-industries/editor/main.js",
        "/home/user/.npm/_npx/a1b2c3d4e5/node_modules/typescript/bin/tsc",
        "/home/user/.npm/_npx/f8e7d6c5b4/node_modules/prettier/bin-prettier.js",
        // The NPX node_modules directory itself
        "/home/user/.npm/_npx/d3b97f1234/node_modules",
        // npm cache (NOT node_modules — should be PackageCache)
        "/home/user/.npm/_cacache/content-v2/sha512/ab/cd/ef",
        "/home/user/.npm/_cacache/index-v5/12/34",
        // yarn cache
        "/home/user/.cache/yarn/v6/npm-lodash-4.17.21/package.json",
        // pnpm store
        "/home/user/.cache/pnpm/store/v3/files/ab/cd",
        // Global node_modules
        "/home/user/.local/lib/node_modules/npm/bin/npm-cli.js",
        "/home/user/.local/lib/node_modules/typescript/bin/tsc",
        // Edge cases: paths with node_modules as substring but not directory
        "/home/user/project/old_node_modules_backup/data.json",
        "/home/user/project/node_modules.old/cache.tmp",
    ];

    eprintln!("\n━━━ NODE_MODULES PIPELINE AUDIT ━━━");
    eprintln!(
        "{:<70} {:<18} {:<22} {:<12} {:<12} {:<8} {:<8} {:<30}",
        "PATH", "RULE", "ENGINE_CAT", "DOMAIN", "INIT_DEC", "FINAL_DEC", "SAFE?", "DOMAIN_KEY"
    );
    eprintln!("{}", "─".repeat(200));

    let mut leaks: Vec<PipelineTrace> = Vec::new();

    for path in &paths {
        let trace = trace_pipeline(path);
        let is_leak = trace.cleanable_safe && trace.domain_key.contains("Node.js");
        if is_leak {
            leaks.push(trace_pipeline(path));
        }
        eprintln!(
            "{:<70} {:<18} {:<22} {:<12} {:<12} {:<12} {:<8} {:<30}",
            &trace.path[..trace.path.len().min(69)],
            trace.matched_rule,
            trace.engine_category,
            trace.cache_domain,
            trace.initial_decision,
            trace.final_decision,
            if trace.cleanable_safe { "YES" } else { "no" },
            trace.domain_key,
        );
    }

    eprintln!("\n━━━ LEAKS (safe-mode cleanable in Node.js domain) ━━━");
    for leak in &leaks {
        eprintln!(
            "  LEAK: {} → rule={} engine={} decision={} tier={}",
            leak.path, leak.matched_rule, leak.engine_category, leak.final_decision, leak.tier
        );
    }
    eprintln!("━━━ {} LEAKS ━━━\n", leaks.len());
}

#[test]
fn audit_cargo_registry_pipeline_trace() {
    let paths = vec![
        // Cargo registry (DownloadedArtifact)
        "/home/user/.cargo/registry/cache/index.crates.io-abc/syn-1.0.crate",
        "/home/user/.cargo/registry/src/index.crates.io-abc/syn-1.0/src/lib.rs",
        "/home/user/.cargo/registry/index.crates.io-6f17d22b50/serde-1.0.228.crate",
        // Cargo git checkouts (BuildCache)
        "/home/user/.cargo/git/checkouts/some-crate-abc123/1a2b3c/src/main.rs",
        "/home/user/.cargo/git/db/some-crate-abc123/.git/HEAD",
        "/home/user/.cargo/git/db/some-crate-abc123/objects/pack/abc.pack",
        // Cargo bin (user binary)
        "/home/user/.cargo/bin/rust-analyzer",
        // Build target (BuildCache)
        "/home/user/project/target/debug/deps/app-abc.rlib",
        "/home/user/project/target/release/build/some-build/output",
        "/home/user/project/target/doc/index.html",
    ];

    eprintln!("\n━━━ CARGO PIPELINE AUDIT ━━━");
    eprintln!(
        "{:<70} {:<18} {:<22} {:<12} {:<12} {:<12} {:<8} {:<30}",
        "PATH", "RULE", "ENGINE_CAT", "DOMAIN", "INIT_DEC", "FINAL_DEC", "SAFE?", "DOMAIN_KEY"
    );
    eprintln!("{}", "─".repeat(200));

    let mut leaks: Vec<PipelineTrace> = Vec::new();

    for path in &paths {
        let trace = trace_pipeline(path);
        let is_leak = trace.cleanable_safe && trace.domain_key.contains("Cargo");
        if is_leak {
            leaks.push(trace_pipeline(path));
        }
        eprintln!(
            "{:<70} {:<18} {:<22} {:<12} {:<12} {:<12} {:<8} {:<30}",
            &trace.path[..trace.path.len().min(69)],
            trace.matched_rule,
            trace.engine_category,
            trace.cache_domain,
            trace.initial_decision,
            trace.final_decision,
            if trace.cleanable_safe { "YES" } else { "no" },
            trace.domain_key,
        );
    }

    eprintln!("\n━━━ LEAKS (safe-mode cleanable in Cargo domain) ━━━");
    for leak in &leaks {
        eprintln!(
            "  LEAK: {} → rule={} engine={} decision={} tier={}",
            leak.path, leak.matched_rule, leak.engine_category, leak.final_decision, leak.tier
        );
    }
    eprintln!("━━━ {} LEAKS ━━━\n", leaks.len());
}
