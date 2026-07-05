// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Shared pipeline helpers — context, classification dispatch, utilities.
//!
//! Extracted from main.rs to keep the entrypoint lean.

use crate::config::Config;
use crate::exclude::ExcludeFilter;
use crate::rules;
use crate::scanner;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Lazily-built set of files currently open by any process.
/// Computed once on first access, reused across commands.
static OPEN_FILES: OnceLock<HashSet<PathBuf>> = OnceLock::new();

pub(crate) fn get_open_files() -> &'static HashSet<PathBuf> {
    OPEN_FILES.get_or_init(crate::procfs::build_open_file_set)
}

pub const BUILD_TARGET: &str = {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
    {
        "linux-amd64-musl"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    {
        "linux-amd64-gnu"
    }
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        not(any(target_env = "musl", target_env = "gnu"))
    ))]
    {
        "linux-amd64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
    {
        "linux-aarch64-musl"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
    {
        "linux-aarch64-gnu"
    }
    #[cfg(all(
        target_os = "linux",
        target_arch = "aarch64",
        not(any(target_env = "musl", target_env = "gnu"))
    ))]
    {
        "linux-aarch64"
    }
    #[cfg(target_os = "freebsd")]
    {
        "freebsd"
    }
    #[cfg(target_os = "openbsd")]
    {
        "openbsd"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "macos"
    )))]
    {
        "unknown"
    }
};

pub struct RunContext {
    pub history_cleaned: HashSet<String>,
    pub health: crate::profiles::HealthMode,
    pub profile: crate::profiles::Profile,
    pub memory: crate::memory::ContextMemory,
}

impl RunContext {
    pub fn new(profile_arg: &str) -> Self {
        RunContext {
            history_cleaned: {
                let h = crate::history::History::load();
                h.previously_cleaned_paths().into_iter().collect()
            },
            health: crate::profiles::detect_health(),
            profile: crate::profiles::Profile::from_str(profile_arg),
            memory: crate::memory::ContextMemory::load(),
        }
    }
}

pub fn print_version() {
    let h = option_env!("ZACXIOM_GIT_HASH").unwrap_or("unknown");
    println!("zacxiom -V/--version");
    println!("Version: v{}", env!("CARGO_PKG_VERSION"));
    println!("Build: {} ({})", BUILD_TARGET, h);
    println!("Copyright: (c) 2026 rezky_nightky (oxyzenQ)");
    println!("License: GPL-3.0");
    println!("Source: https://github.com/oxyzenQ/zacxiom");
}

pub fn resolve_roots(paths: Vec<String>) -> Vec<PathBuf> {
    if paths.is_empty() {
        scanner::default_scan_roots()
    } else {
        paths.into_iter().map(PathBuf::from).collect()
    }
}

/// v13: Build the effective exclude filter from config + CLI flags.
/// CLI excludes are appended to config excludes.
pub fn build_exclude_filter(cfg: &Config, cli_exclude: &[String]) -> ExcludeFilter {
    match ExcludeFilter::build(&cfg.scan.exclude, &cfg.scan.exclude_patterns, cli_exclude) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: invalid exclude pattern in config or CLI: {e}");
            eprintln!("  Continuing with config-only excludes.");
            ExcludeFilter::build(&cfg.scan.exclude, &cfg.scan.exclude_patterns, &[])
                .unwrap_or_else(|_| ExcludeFilter::empty())
        }
    }
}

/// Determine optimal thread count based on workload size.
/// v13: "Peak but efficient" — uses 75% of CPUs, scaled by workload, load-aware.
///
/// Philosophy: zacxiom is a master of threading — boost peak performance when
/// scanning large filesets, but never hog the CPU. Leaves 25% headroom for the
/// system and reduces threads if /proc/loadavg indicates high system load.
///
/// - Small workloads (<100 files): 2 threads (overhead > benefit)
/// - Medium (<10k): 50% of headroom
/// - Large (<100k): full headroom
/// - Massive (>=100k): full headroom
/// - Load-aware: if 1-min load avg > 70% of CPUs, halve threads
pub fn optimal_threads(file_count: usize) -> usize {
    optimal_threads_with_config(file_count, 0)
}

/// v13: Configurable thread count. If max_threads=0, auto-compute.
/// If max_threads > 0, use that (clamped to available CPUs).
pub fn optimal_threads_with_config(file_count: usize, max_threads: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // Manual override — respect user's config but clamp to CPUs
    if max_threads > 0 {
        return max_threads.min(cpus).max(1);
    }

    // Auto mode: 75% of CPUs, leave 25% headroom for system
    let headroom = (cpus * 3 / 4).max(2);

    // Scale threads by workload size — small workloads don't benefit from many threads
    let scaled = if file_count < 100 {
        2 // overhead > benefit for tiny workloads
    } else if file_count < 1_000 {
        (headroom / 4).max(2)
    } else if file_count < 10_000 {
        (headroom / 2).max(2)
    } else {
        headroom // large workloads get full headroom
    };

    // Load-aware: check /proc/loadavg, reduce threads if system is busy
    let load_adjusted = apply_load_aware_scaling(scaled, cpus);

    load_adjusted.min(headroom).max(1)
}

/// v13: Read /proc/loadavg and reduce thread count if system is under load.
/// v14.0: Cross-Unix — on non-Linux, returns threads unchanged (no /proc).
/// If 1-min load average > 70% of CPU count, halve the threads.
/// This prevents zacxiom from being a "CPU monster" on busy systems.
fn apply_load_aware_scaling(threads: usize, cpus: usize) -> usize {
    if threads <= 2 {
        return threads; // already minimal
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(loadavg_str) = std::fs::read_to_string("/proc/loadavg") {
            // Format: "0.52 0.48 0.45 1/234 5678"
            if let Some(first) = loadavg_str.split_whitespace().next() {
                if let Ok(load1) = first.parse::<f64>() {
                    let threshold = cpus as f64 * 0.7;
                    if load1 > threshold {
                        // System is busy — halve our threads
                        return (threads / 2).max(2);
                    }
                }
            }
        }
    }
    // Non-Linux or /proc unavailable — return threads unchanged
    threads
}

pub fn classify(
    entries: Vec<scanner::ScanEntry>,
    ctx: &RunContext,
    threads: usize,
    cfg: &Config,
    cache: &crate::scan_cache::ScanCache,
) -> Vec<rules::ClassifiedFile> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // v14.1: Reset cache hit/miss counters for this classify session
    crate::scan_cache::reset_stats();

    let total = entries.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let ctr = counter.clone();

    // Progress reporter thread for large datasets
    // v13.1: Includes ETA based on scan rate
    let _reporter = if total > 500 {
        let start = std::time::Instant::now();
        Some(std::thread::spawn(move || {
            loop {
                let done = ctr.load(Ordering::Relaxed);
                if done >= total {
                    break;
                }
                let pct = done * 100 / total;
                let bar = 20;
                let filled = pct * bar / 100;
                let done_str = if done >= 1_000_000 {
                    format!("{:.1}M", done as f64 / 1_000_000.0)
                } else if done >= 1_000 {
                    format!("{:.1}K", done as f64 / 1_000.0)
                } else {
                    format!("{done}")
                };
                let total_str = if total >= 1_000_000 {
                    format!("{:.1}M", total as f64 / 1_000_000.0)
                } else if total >= 1_000 {
                    format!("{:.1}K", total as f64 / 1_000.0)
                } else {
                    format!("{total}")
                };
                // v13.1: ETA calculation
                let elapsed = start.elapsed().as_secs_f64();
                let eta_str = if done > 0 && elapsed > 0.5 {
                    let rate = done as f64 / elapsed;
                    let remaining = (total - done) as f64 / rate;
                    if remaining < 1.0 {
                        "<1s".to_string()
                    } else if remaining < 60.0 {
                        format!("~{:.0}s", remaining.ceil())
                    } else if remaining < 3600.0 {
                        format!("~{:.0}m", (remaining / 60.0).ceil())
                    } else {
                        format!("~{:.1}h", remaining / 3600.0)
                    }
                } else {
                    "ETA ...".to_string()
                };
                print!(
                    "\r\x1b[K  {} [{:5}] {:>7} / {:<7}  [{}{}] {:>3}% {}",
                    crate::color::purple_spinner('⠋'),
                    "CLASSIFY",
                    done_str,
                    total_str,
                    "█".repeat(filled),
                    "░".repeat(bar.saturating_sub(filled)),
                    pct,
                    eta_str,
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            print!("\r\x1b[K");
            std::io::stdout().flush().ok();
        }))
    } else {
        None
    };

    // v13: Build rayon pool with graceful fallback — never panic on thread creation failure.
    // If pool build fails (rare: resource limits), fall back to sequential iteration.
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build();
    let result = match pool {
        Ok(p) => p.install(|| {
            entries
                .into_par_iter()
                .map(|e| {
                    let path_str = e.path.to_string_lossy().into_owned();

                    // v14.1: Cache-aware classification — skip full pipeline if unchanged.
                    // Check (path, size, mtime) in cache. If hit, reuse stored decision.
                    let mtime = crate::scan_cache::get_mtime_secs(&e.path).unwrap_or(0);
                    if let Some(cached) = cache.check_hit(&path_str, e.size, mtime) {
                        // CACHE HIT: build ClassifiedFile from cached result, skip classification
                        let decision = parse_decision(&cached.decision)
                            .unwrap_or(rules::Decision::Moderate);
                        let domain = parse_domain(&cached.cache_domain)
                            .unwrap_or(rules::CacheDomain::Unknown);
                        counter.fetch_add(1, Ordering::Relaxed);
                        return rules::ClassifiedFile {
                            path: path_str,
                            size: e.size,
                            cache_domain: domain,
                            ownership: rules::Ownership::User { uid: 1000 },
                            risk_score: cached.risk_score,
                            risk_reasons: vec!["cached classification (unchanged)".into()],
                            decision,
                            engine_category: cached.engine_category.clone(),
                            engine_confidence: cached.engine_confidence,
                        };
                    }

                    // CACHE MISS: run full classification pipeline
                    let d = crate::cache::classify(&e.path);
                    let o = crate::ownership::detect(&e.path);
                    let age = crate::risk::file_age_days(&path_str);
                    let modif = ctx.memory.risk_modifier(&path_str);
                    let mut scored = crate::risk::score_v3(&crate::risk::RiskSignals {
                        path: &path_str,
                        size: e.size,
                        domain: &d,
                        ownership: &o,
                        open_files: Some(get_open_files()),
                        history_cleaned: Some(&ctx.history_cleaned),
                        memory_modifier: modif,
                        age_days: age,
                    });
                    if modif != 0.0 {
                        scored.risk_reasons.push(format!(
                            "memory: adaptive modifier {modif:+.3} (sessions: {})",
                            ctx.memory.sessions
                        ));
                    }
                    // v6.3.1: bridge — fast classify, zero-heap category
                    let eng = crate::engine::classify_fast(&e.path);
                    scored.engine_category = eng.0.to_string();
                    scored.engine_confidence = eng.1;

                    // v7: Bridge — engine category overrides legacy Decision
                    // to align semantic identity with cleanup policy.
                    // Toolchain, installed software, dependency source, and
                    // downloaded artifacts all require --smart — not auto-cleanable.
                    if scored.decision == rules::Decision::Safe {
                        if eng.0 == "Toolchain Installation"
                            || eng.0 == "Toolchain Manager"
                            || eng.0 == "Installed Software"
                            || eng.0 == "Dependency Source"
                        {
                            scored.decision = rules::Decision::LowRisk;
                            scored.risk_reasons.push(
                                "Not disposable cache — regenerable but expensive to restore, requires --smart".into(),
                            );
                        }
                        // Downloaded artifacts (cargo registry, SDKs) — also need --smart
                        else if eng.0.contains("Downloaded") {
                            scored.decision = rules::Decision::LowRisk;
                            scored.risk_reasons.push(
                                "Downloaded artifact: regenerable but expensive to restore".into(),
                            );
                        }
                    }

                    // v13: Engine-protected category enforcement.
                    // If the engine classifies a file as a protected category
                    // (System Data, Security Credential, Project Workspace, etc.),
                    // override ANY decision to Protected — never cleanable.
                    // FIX: .git/HEAD was classified as Moderate by risk scoring
                    // but engine said "System Data" → should be Protected.
                    if matches!(
                        eng.0,
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
                        scored.decision = rules::Decision::Protected;
                        scored.risk_reasons.push(format!(
                            "Engine classified as protected: {} — never cleanable",
                            eng.0
                        ));
                        scored.risk_score = 1.0;
                    }

                    // v11: Active Environment Protection
                    // Applies to ALL decisions, not just Safe.
                    // Override ANY decision to ProtectedActiveEnvironment
                    // if the file is inside an active SDK, toolchain, or runtime.
                    // Must run AFTER toolchain/LowRisk overrides above.
                    if let Some(active_env) = crate::environment::is_active_environment(&e.path) {
                        scored.decision = rules::Decision::ProtectedActiveEnvironment;
                        scored.risk_reasons.push(format!(
                            "Active environment: {} — {}",
                            active_env.name, active_env.stack
                        ));
                        scored.risk_score = 1.0;
                    }
                    // v13: Rules-based exclude — config-driven patterns from [rules_exclude].exclude.
                    // Replaces hardcoded PROTECTED_EXTENSIONS. Files matching these patterns
                    // are NEVER cleanable, regardless of location.
                    // Also checks legacy [clean].protect_extensions and protect_patterns for backward compat.
                    let path_obj = std::path::Path::new(&path_str);
                    let is_excluded = rules::matches_rules_exclude(path_obj, &cfg.rules_exclude.exclude)
                        || rules::has_protected_extension(path_obj)  // legacy fallback
                        || rules::matches_protected_pattern(path_obj, &cfg.clean.protect_patterns);
                    if is_excluded {
                        scored.decision = rules::Decision::Protected;
                        scored.risk_reasons.push(
                            "Rules-excluded file (disk image / crypto key / user pattern) — never cleanable".into(),
                        );
                        scored.risk_score = 1.0;
                    }

                    // v13: Size-based protection — large files in user-content dirs need explicit --force
                    if scored.decision == rules::Decision::Safe
                        && scored.size > cfg.clean.max_auto_clean_size
                        && crate::scanner::is_user_content_dir(path_obj)
                    {
                        scored.decision = rules::Decision::Moderate;
                        scored.risk_reasons.push(format!(
                            "Large file ({}) in user directory — requires --force",
                            crate::simulator::human_size(scored.size)
                        ));
                    }

                    // v14.2: Age-based auto-clean policy.
                    // If [clean].auto_clean_older_than_days > 0 and file is older than that,
                    // upgrade decision to Safe (auto-cleanable) — unless it's Protected.
                    if cfg.clean.auto_clean_older_than_days > 0 {
                        if let Some(age_val) = age {
                            if age_val > cfg.clean.auto_clean_older_than_days as f64
                                && !matches!(
                                    scored.decision,
                                    rules::Decision::Protected
                                        | rules::Decision::ProtectedActiveEnvironment
                                )
                            {
                                scored.decision = rules::Decision::Safe;
                                scored.risk_reasons.push(format!(
                                    "Auto-clean: aged {:.0}d (>{}d threshold)",
                                    age_val, cfg.clean.auto_clean_older_than_days
                                ));
                            }
                        }
                    }

                    counter.fetch_add(1, Ordering::Relaxed);
                    scored
                })
                .collect()
        }),
        Err(e) => {
            // Pool build failed — fall back to sequential iteration.
            // This is rare (only on systems with severe resource limits) but we must never panic.
            eprintln!("Warning: thread pool creation failed ({e}), falling back to single-threaded mode");
            entries
                .into_iter()
                .map(|e| {
                    let path_str = e.path.to_string_lossy().into_owned();

                    // v14.1: Cache-aware classification (sequential fallback)
                    let mtime = crate::scan_cache::get_mtime_secs(&e.path).unwrap_or(0);
                    if let Some(cached) = cache.check_hit(&path_str, e.size, mtime) {
                        let decision = parse_decision(&cached.decision)
                            .unwrap_or(rules::Decision::Moderate);
                        let domain = parse_domain(&cached.cache_domain)
                            .unwrap_or(rules::CacheDomain::Unknown);
                        counter.fetch_add(1, Ordering::Relaxed);
                        return rules::ClassifiedFile {
                            path: path_str,
                            size: e.size,
                            cache_domain: domain,
                            ownership: rules::Ownership::User { uid: 1000 },
                            risk_score: cached.risk_score,
                            risk_reasons: vec!["cached classification (unchanged)".into()],
                            decision,
                            engine_category: cached.engine_category.clone(),
                            engine_confidence: cached.engine_confidence,
                        };
                    }

                    let d = crate::cache::classify(&e.path);
                    let o = crate::ownership::detect(&e.path);
                    let age = crate::risk::file_age_days(&path_str);
                    let modif = ctx.memory.risk_modifier(&path_str);
                    let mut scored = crate::risk::score_v3(&crate::risk::RiskSignals {
                        path: &path_str,
                        size: e.size,
                        domain: &d,
                        ownership: &o,
                        open_files: Some(get_open_files()),
                        history_cleaned: Some(&ctx.history_cleaned),
                        memory_modifier: modif,
                        age_days: age,
                    });
                    if modif != 0.0 {
                        scored.risk_reasons.push(format!(
                            "memory: adaptive modifier {modif:+.3} (sessions: {})",
                            ctx.memory.sessions
                        ));
                    }
                    let eng = crate::engine::classify_fast(&e.path);
                    scored.engine_category = eng.0.to_string();
                    scored.engine_confidence = eng.1;

                    if scored.decision == rules::Decision::Safe {
                        if eng.0 == "Toolchain Installation"
                            || eng.0 == "Toolchain Manager"
                            || eng.0 == "Installed Software"
                            || eng.0 == "Dependency Source"
                        {
                            scored.decision = rules::Decision::LowRisk;
                            scored.risk_reasons.push(
                                "Not disposable cache — regenerable but expensive to restore, requires --smart".into(),
                            );
                        }
                        else if eng.0.contains("Downloaded") {
                            scored.decision = rules::Decision::LowRisk;
                            scored.risk_reasons.push(
                                "Downloaded artifact: regenerable but expensive to restore".into(),
                            );
                        }
                    }

                    if matches!(
                        eng.0,
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
                        scored.decision = rules::Decision::Protected;
                        scored.risk_reasons.push(format!(
                            "Engine classified as protected: {} — never cleanable",
                            eng.0
                        ));
                        scored.risk_score = 1.0;
                    }

                    if let Some(active_env) = crate::environment::is_active_environment(&e.path) {
                        scored.decision = rules::Decision::ProtectedActiveEnvironment;
                        scored.risk_reasons.push(format!(
                            "Active environment: {} — {}",
                            active_env.name, active_env.stack
                        ));
                        scored.risk_score = 1.0;
                    }

                    let path_obj = std::path::Path::new(&path_str);
                    let is_excluded = rules::matches_rules_exclude(path_obj, &cfg.rules_exclude.exclude)
                        || rules::has_protected_extension(path_obj)
                        || rules::matches_protected_pattern(path_obj, &cfg.clean.protect_patterns);
                    if is_excluded {
                        scored.decision = rules::Decision::Protected;
                        scored.risk_reasons.push(
                            "Rules-excluded file (disk image / crypto key / user pattern) — never cleanable".into(),
                        );
                        scored.risk_score = 1.0;
                    }

                    if scored.decision == rules::Decision::Safe
                        && scored.size > cfg.clean.max_auto_clean_size
                        && crate::scanner::is_user_content_dir(path_obj)
                    {
                        scored.decision = rules::Decision::Moderate;
                        scored.risk_reasons.push(format!(
                            "Large file ({}) in user directory — requires --force",
                            crate::simulator::human_size(scored.size)
                        ));
                    }

                    counter.fetch_add(1, Ordering::Relaxed);
                    scored
                })
                .collect()
        }
    };
    result
}

/// v14.1: Parse Decision enum from cache string (Debug format).
fn parse_decision(s: &str) -> Option<rules::Decision> {
    match s {
        "Safe" => Some(rules::Decision::Safe),
        "LowRisk" => Some(rules::Decision::LowRisk),
        "Moderate" => Some(rules::Decision::Moderate),
        "HighRisk" => Some(rules::Decision::HighRisk),
        "Protected" => Some(rules::Decision::Protected),
        "ProtectedActiveEnvironment" => Some(rules::Decision::ProtectedActiveEnvironment),
        _ => None,
    }
}

/// v14.1: Parse CacheDomain enum from cache string (Display format).
fn parse_domain(s: &str) -> Option<rules::CacheDomain> {
    match s {
        "browser" => Some(rules::CacheDomain::Browser),
        "system" => Some(rules::CacheDomain::System),
        "build_artifact" => Some(rules::CacheDomain::BuildArtifact),
        "package_manager" => Some(rules::CacheDomain::PackageManager),
        "developer" => Some(rules::CacheDomain::Developer),
        "user_data" => Some(rules::CacheDomain::UserData),
        "unknown" => Some(rules::CacheDomain::Unknown),
        _ => None,
    }
}

pub fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let tod = secs % 86400;
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let diy = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366
        } else {
            365
        };
        if d < diy {
            break;
        }
        d -= diy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mdays: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut rem = d;
    let mut mo = 1i64;
    for &md in &mdays {
        if rem < md {
            break;
        }
        rem -= md;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{:02}T{h:02}:{m:02}:{s:02}Z", rem + 1)
}

/// Extract top storage contributors from classified files (v6.2.1).
/// Groups by path prefix patterns to show "where storage is going".
pub fn top_contributors(
    files: &[rules::ClassifiedFile],
    limit: usize,
) -> Vec<(String, usize, u64)> {
    use std::collections::HashMap;

    // Group by path-derived contributor name
    let mut groups: HashMap<String, (usize, u64)> = HashMap::new();

    for f in files {
        let name = contributor_name(&f.path);
        let entry = groups.entry(name).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += f.size;
    }

    let mut sorted: Vec<(String, usize, u64)> =
        groups.into_iter().map(|(k, (c, s))| (k, c, s)).collect();
    sorted.sort_by_key(|(_, _, s)| std::cmp::Reverse(*s));
    sorted.truncate(limit);
    sorted
}

/// Derive a human-readable contributor name from a file path.
pub fn contributor_name(path: &str) -> String {
    let lower = path.to_lowercase();

    // Browser-specific
    if lower.contains("firefox") || lower.contains("mozilla") {
        return "Firefox".into();
    }
    if lower.contains("chromium") {
        return "Chromium".into();
    }
    if lower.contains("chrome") {
        return "Google Chrome".into();
    }
    if lower.contains("brave") {
        return "Brave".into();
    }
    if lower.contains("edge") {
        return "Microsoft Edge".into();
    }

    // Developer tools
    if lower.contains(".cargo") {
        return "Cargo (Rust)".into();
    }
    if lower.contains("rustup") {
        return "Rustup".into();
    }
    if lower.contains(".npm") || lower.contains("npm") {
        return "npm".into();
    }
    if lower.contains("pnpm") {
        return "pnpm".into();
    }
    if lower.contains("yarn") {
        return "Yarn".into();
    }
    if lower.contains("pip") {
        return "pip (Python)".into();
    }
    if lower.contains("/uv/") || lower.contains(".cache/uv") {
        return "uv (Python)".into();
    }
    if lower.contains("docker") || lower.contains("containers") {
        return "Docker".into();
    }
    if lower.contains("gradle") {
        return "Gradle".into();
    }
    if lower.contains("maven") || lower.contains(".m2") {
        return "Maven".into();
    }
    if lower.contains("node_modules") {
        return "Node.js (node_modules)".into();
    }

    // Gaming
    if lower.contains("steam") {
        return "Steam".into();
    }
    if lower.contains("lutris") {
        return "Lutris".into();
    }
    if lower.contains("heroic") {
        return "Heroic".into();
    }
    if lower.contains("compatdata") || lower.contains("proton") {
        return "Proton (Steam)".into();
    }
    if lower.contains("dxvk") || lower.contains("vkd3d") || lower.contains("mesa") {
        return "Shader Cache".into();
    }

    // Desktop apps
    if lower.contains("discord") {
        return "Discord".into();
    }
    if lower.contains("spotify") {
        return "Spotify".into();
    }
    if lower.contains("slack") {
        return "Slack".into();
    }
    if lower.contains("vscode") || lower.contains("visual studio") {
        return "VS Code".into();
    }
    if lower.contains("jetbrains") || lower.contains("intellij") {
        return "JetBrains IDE".into();
    }
    if lower.contains("thunderbird") {
        return "Thunderbird".into();
    }

    // AI/ML
    if lower.contains("huggingface") {
        return "HuggingFace".into();
    }
    if lower.contains("ollama") {
        return "Ollama".into();
    }
    if lower.contains("torch") || lower.contains("pytorch") {
        return "PyTorch".into();
    }

    // System
    if lower.contains("/tmp/") {
        return "Temporary Files".into();
    }
    if lower.contains("trash") {
        return "Desktop Trash".into();
    }
    if lower.contains("downloads") {
        return "Downloads".into();
    }
    if lower.contains("pacman") || lower.contains("yay") || lower.contains("paru") {
        return "Package Manager".into();
    }

    // Fallback: extract app name from the segment AFTER .cache/.config/.local
    let segments: Vec<&str> = path.split('/').collect();
    for i in 0..segments.len().saturating_sub(1) {
        let seg = segments[i].to_lowercase();
        if (seg == ".cache" || seg == ".config" || seg == ".local") && i + 1 < segments.len() {
            let name = segments[i + 1];
            if !name.is_empty() && !name.starts_with('.') {
                let capitalized = {
                    let mut chars = name.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                };
                return capitalized;
            }
        }
    }
    "Other".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contributor_name_known_browsers() {
        assert_eq!(
            contributor_name("/home/user/.cache/mozilla/firefox/x"),
            "Firefox"
        );
        assert_eq!(
            contributor_name("/home/user/.cache/chromium/Default/Cache"),
            "Chromium"
        );
        assert_eq!(
            contributor_name("/home/user/.cache/google-chrome/"),
            "Google Chrome"
        );
    }

    #[test]
    fn test_contributor_name_developer_tools() {
        assert_eq!(
            contributor_name("/home/user/.cargo/registry/cache/x"),
            "Cargo (Rust)"
        );
        assert_eq!(
            contributor_name("/home/user/.rustup/toolchains/stable"),
            "Rustup"
        );
        assert_eq!(contributor_name("/home/user/.npm/_cacache/x"), "npm");
    }

    #[test]
    fn test_contributor_name_fallback_extracts_app() {
        // Fallback should return the directory name AFTER .cache/.config/.local
        // Use app names that do NOT match any explicit checks above
        assert_eq!(
            contributor_name("/home/user/.cache/ghostty/cache-data"),
            "Ghostty"
        );
        assert_eq!(
            contributor_name("/home/user/.config/wezterm/prefs"),
            "Wezterm"
        );
    }

    #[test]
    fn test_contributor_name_fallback_other_when_no_match() {
        assert_eq!(contributor_name("/opt/something/weird/path"), "Other");
    }

    #[test]
    fn test_contributor_name_fallback_skips_dot_dirs() {
        assert_eq!(
            contributor_name("/home/user/.cache/.hidden/app/data"),
            "Other"
        );
    }
}
