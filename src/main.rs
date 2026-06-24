// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine v6.2.0
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.
//! v6.2.3: Dynamic multithreading + purple accent styling.
#![allow(dead_code)]

mod cache;
mod cleaner;
mod cli;
mod color;
mod confidence;
mod dependency;
mod discovery;
mod display;
mod domain;
mod engine;
mod errors;
mod explain;
mod history;
mod impact;
mod inspect;
mod memory;
mod ownership;
mod policy;
mod procfs;
mod profiles;
mod progress;
mod risk;
mod rules;
mod safety;
mod scanner;
mod simulator;
mod snapshot;
mod summary;

use clap::Parser;
use cli::{Cli, Command};
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;

const BUILD_TARGET: &str = {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "linux-aarch64"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64")
    )))]
    {
        "unknown"
    }
};

struct RunContext {
    open_files: HashSet<PathBuf>,
    history_cleaned: HashSet<String>,
    health: profiles::HealthMode,
    profile: profiles::Profile,
    memory: memory::ContextMemory,
}

impl RunContext {
    fn new(profile_arg: &str) -> Self {
        RunContext {
            open_files: procfs::build_open_file_set(),
            history_cleaned: {
                let h = history::History::load();
                h.previously_cleaned_paths().into_iter().collect()
            },
            health: profiles::detect_health(),
            profile: profiles::Profile::from_str(profile_arg),
            memory: memory::ContextMemory::load(),
        }
    }
}

fn main() {
    color::init();
    let cli = Cli::parse();
    if cli.version {
        print_version();
        return;
    }
    if cli.check_update {
        check_update();
        return;
    }

    let command = cli.command.unwrap_or_else(|| {
        eprintln!("No command specified. Use --help for usage.");
        std::process::exit(1);
    });

    match command {
        Command::Scan {
            paths,
            depth,
            min_size,
            profile,
            json,
        } => run_scan(paths, depth, min_size, json, false, &profile),

        Command::Report {
            paths,
            depth,
            profile,
            json,
        } => run_scan(paths, depth, 1, json, true, &profile),

        Command::Simulate {
            paths,
            depth,
            profile,
            json,
            verbose,
        } => run_simulate(paths, depth, json, verbose, &profile),

        Command::Clean {
            paths,
            depth,
            profile,
            smart,
            force,
            dry_run,
            verbose,
            json,
        } => run_clean(paths, depth, smart, force, dry_run, verbose, json, &profile),

        Command::Explain { target, path } => {
            let target_path = path.as_deref().unwrap_or(&target);
            if target_path.is_empty() {
                eprintln!("Usage: zacxiom explain <path>");
                eprintln!("       zacxiom explain --path <path>");
                std::process::exit(1);
            }
            run_explain(target_path);
        }

        Command::CheckUpdate => check_update(),
        Command::Undo { id } => run_undo(id),
        Command::Status => run_status(),
        Command::InspectUnknown {
            paths,
            depth,
            json,
            verbose,
        } => run_inspect_unknown(paths, depth, json, verbose),
    }
}

fn print_version() {
    let h = option_env!("ZACXIOM_GIT_HASH").unwrap_or("unknown");
    println!("zacxiom -V/--version");
    println!("Version: v{}", env!("CARGO_PKG_VERSION"));
    println!("Build: {} ({})", BUILD_TARGET, h);
    println!("Copyright: (c) 2026 rezky_nightky (oxyzenQ)");
    println!("License: GPL-3.0");
    println!("Source: https://github.com/oxyzenQ/zacxiom");
}

fn check_update() {
    use std::process::Command;

    const GITHUB_API_URL: &str = "https://api.github.com/repos/oxyzenQ/zacxiom/releases/latest";
    const RELEASES_URL: &str = "https://github.com/oxyzenQ/zacxiom/releases/latest";
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    #[derive(Debug, PartialEq, Eq)]
    enum UpdateStatus {
        UpToDate,
        UpdateAvailable,
        CurrentIsNewer,
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct SemVer {
        major: u64,
        minor: u64,
        patch: u64,
    }

    impl SemVer {
        fn parse(version: &str) -> Option<Self> {
            let version = version.trim();
            let version = version.strip_prefix('v').unwrap_or(version);
            let version = version
                .split_once('-')
                .map_or(version, |(stable, _)| stable);
            let mut parts = version.split('.');
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next()?.parse().ok()?;
            let patch = parts.next()?.parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            Some(Self {
                major,
                minor,
                patch,
            })
        }
    }

    fn normalize_version(version: &str) -> String {
        let version = version.trim();
        if version.starts_with('v') {
            version.to_string()
        } else {
            format!("v{version}")
        }
    }

    fn compare_versions(current: &str, latest: &str) -> UpdateStatus {
        match (SemVer::parse(current), SemVer::parse(latest)) {
            (Some(current), Some(latest)) if current == latest => UpdateStatus::UpToDate,
            (Some(current), Some(latest)) if current > latest => UpdateStatus::CurrentIsNewer,
            _ => UpdateStatus::UpdateAvailable,
        }
    }

    fn extract_tag_name(json: &str) -> Option<String> {
        let key = "\"tag_name\"";
        let rest = json.get(json.find(key)? + key.len()..)?;
        let rest = rest.trim_start().strip_prefix(':')?.trim_start();
        let rest = rest.strip_prefix('"')?;
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn interpret_curl_exit(code: i32) -> &'static str {
        match code {
            6 => "DNS resolution failed",
            7 => "connection refused",
            28 => "network request timed out",
            35 => "SSL/TLS handshake failed",
            _ => "network request failed",
        }
    }

    fn interpret_http_status(code: u16) -> &'static str {
        match code {
            403 => "GitHub API request was rate-limited or forbidden",
            404 => "no latest GitHub release found for oxyzenQ/zacxiom",
            _ => "GitHub API returned an unexpected error",
        }
    }

    let output = Command::new("curl")
        .args([
            "--silent",
            "--max-time",
            "15",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: zacxiom-check-update",
            "--write-out",
            "\n%{http_code}",
            GITHUB_API_URL,
        ])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("zacxiom update check failed: curl is not available on PATH");
            } else {
                eprintln!("zacxiom update check failed: {e}");
            }
            return;
        }
    };

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        eprintln!("zacxiom update check failed: {}", interpret_curl_exit(code));
        return;
    }

    let raw = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("zacxiom update check failed: response was not valid UTF-8");
            return;
        }
    };

    let (body, status_str) = match raw.rsplit_once('\n') {
        Some(pair) => pair,
        None => {
            eprintln!("zacxiom update check failed: GitHub API response was malformed");
            return;
        }
    };
    let status: u16 = status_str.trim().parse().unwrap_or(0);
    if status != 200 {
        eprintln!(
            "zacxiom update check failed: {}",
            interpret_http_status(status)
        );
        return;
    }

    let latest_tag = match extract_tag_name(body) {
        Some(t) => t,
        None => {
            eprintln!("zacxiom update check failed: could not parse latest release tag from GitHub response");
            return;
        }
    };

    let status_text = match compare_versions(CURRENT_VERSION, &latest_tag) {
        UpdateStatus::UpToDate => "up to date",
        UpdateStatus::UpdateAvailable => "update available",
        UpdateStatus::CurrentIsNewer => "current is newer than latest release",
    };

    println!("zacxiom update check");
    println!("Current: {}", normalize_version(CURRENT_VERSION));
    println!("Latest:  {}", normalize_version(&latest_tag));
    println!("Status:  {status_text}");
    println!("Source:  {RELEASES_URL}");
}

fn resolve_roots(paths: Vec<String>) -> Vec<PathBuf> {
    if paths.is_empty() {
        scanner::default_scan_roots()
    } else {
        paths.into_iter().map(PathBuf::from).collect()
    }
}

/// Determine optimal thread count based on workload size.
/// Small: 2 threads. Medium: half of logical CPUs. Large: all CPUs.
fn optimal_threads(file_count: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    if file_count < 5_000 {
        2.min(cpus)
    } else if file_count < 50_000 {
        (cpus / 2).max(2)
    } else {
        cpus.max(2)
    }
}

fn classify(
    entries: Vec<scanner::ScanEntry>,
    ctx: &RunContext,
    threads: usize,
) -> Vec<rules::ClassifiedFile> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let total = entries.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let ctr = counter.clone();

    // Progress reporter thread for large datasets
    let _reporter = if total > 500 {
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
                print!(
                    "\r\x1b[K  {} [{:5}] {:>7} / {:<7}  [{}{}] {:>3}%",
                    crate::color::purple_spinner('⠋'),
                    "CLASSIFY",
                    done_str,
                    total_str,
                    "█".repeat(filled),
                    "░".repeat(bar.saturating_sub(filled)),
                    pct,
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            print!("\r\x1b[K");
            std::io::stdout().flush().ok();
        }))
    } else {
        None
    };

    let result = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("rayon pool")
        .install(|| {
            entries
                .into_par_iter()
                .map(|e| {
                    let d = cache::classify(&e.path);
                    let o = ownership::detect(&e.path);
                    let path_str = e.path.to_string_lossy().into_owned();
                    let age = risk::file_age_days(&path_str);
                    let modif = ctx.memory.risk_modifier(&path_str);
                    let mut scored = risk::score_v3(&risk::RiskSignals {
                        path: &path_str,
                        size: e.size,
                        domain: &d,
                        ownership: &o,
                        open_files: Some(&ctx.open_files),
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
                    counter.fetch_add(1, Ordering::Relaxed);
                    scored
                })
                .collect()
        });
    result
}

fn run_scan(
    paths: Vec<String>,
    depth: usize,
    min_size: u64,
    json: bool,
    verbose: bool,
    profile: &str,
) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, min_size, true);
    prog.advance();
    let threads = optimal_threads(entries.len());
    prog.set_threads(threads);
    let classified = classify(entries, &ctx, threads);
    prog.advance();
    prog.advance();
    prog.done();

    if json {
        let out = serde_json::json!({
            "health": format!("{:?}", ctx.health),
            "profile": format!("{:?}", ctx.profile),
            "domains": domain::summarize(&classified),
            "files": classified,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }

    let ds = summary::DecisionSummary::from_files(&classified, ctx.open_files.len());
    println!("\n{}", display::render_decision_summary(&ds));

    let domains = domain::summarize(&classified);
    println!("{}", display::render_domain_summary(&domains));

    // v6.2.0: show confidence breakdown
    let cs = confidence::ConfidenceSummary::from_files(&classified);
    println!("{}", display::render_confidence_summary(&cs));

    // v6.3.2: classifier coverage
    let cov = inspect::analyze(&classified);
    println!("{}", inspect::render_coverage(&cov));

    if verbose {
        println!("{}", display::render_table(&classified, "FILE DETAIL"));
    }
}

fn run_simulate(paths: Vec<String>, depth: usize, json: bool, verbose: bool, profile: &str) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    prog.advance();
    let threads = optimal_threads(entries.len());
    prog.set_threads(threads);
    let classified = classify(entries, &ctx, threads);
    prog.advance();
    prog.advance();
    prog.done();

    if json {
        let report = simulator::simulate(&classified, &ctx.health, &ctx.profile);
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    let ds = summary::DecisionSummary::from_files(&classified, ctx.open_files.len());
    println!("\n{}", display::render_decision_summary(&ds));

    let domains = domain::summarize(&classified);
    println!("{}", display::render_domain_summary(&domains));

    let cs = confidence::ConfidenceSummary::from_files(&classified);
    println!("{}", display::render_confidence_summary(&cs));

    // v6.3.2: classifier coverage
    let cov = inspect::analyze(&classified);
    println!("{}", inspect::render_coverage(&cov));

    let _report = simulator::simulate(&classified, &ctx.health, &ctx.profile);

    // v6.2.4: default summary mode — domain + top contributors only
    if verbose {
        println!("{}", display::render_simulation(&classified, "SIMULATION"));
    } else {
        let cleanable: Vec<_> = classified
            .iter()
            .filter(|f| matches!(f.decision, rules::Decision::Safe | rules::Decision::LowRisk))
            .collect();
        let domains = domain::summarize(&classified);
        if !domains.is_empty() {
            println!("{}", display::render_domains_compact(&domains));
        }
        if !cleanable.is_empty() {
            // Top contributors
            let top = top_contributors(
                &cleanable.iter().map(|f| (*f).clone()).collect::<Vec<_>>(),
                8,
            );
            if !top.is_empty() {
                println!("\nTOP CONTRIBUTORS\n{}", "─".repeat(40));
                for (name, count, size) in &top {
                    println!(
                        "  {:<38} {:>4} files  {}",
                        name,
                        count,
                        simulator::human_size(*size)
                    );
                }
            }
            // Top 20 largest
            let mut sorted: Vec<_> = cleanable.iter().collect();
            sorted.sort_by_key(|f| std::cmp::Reverse(f.size));
            println!("\nLARGEST CLEANABLE\n{}", "─".repeat(40));
            for f in sorted.iter().take(20) {
                let tier = confidence::confidence(f);
                println!(
                    "  {} {}  {}",
                    tier.stars(),
                    simulator::human_size(f.size),
                    f.path
                );
            }
        }
        println!("\n  (use --verbose for full file list)");
    }
}

#[allow(clippy::too_many_arguments)]
fn run_clean(
    paths: Vec<String>,
    depth: usize,
    smart: bool,
    force: bool,
    dry_run: bool,
    verbose: bool,
    json: bool,
    profile: &str,
) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    prog.advance();
    let threads = optimal_threads(entries.len());
    prog.set_threads(threads);
    let classified = classify(entries, &ctx, threads);
    prog.advance();
    prog.advance();
    prog.done();

    if dry_run {
        // v6.2.2: clean summary, no box art
        let cleanable: Vec<_> = classified
            .iter()
            .filter(|f| f.decision.is_cleanable(smart, force))
            .collect();
        let skipped: Vec<_> = classified
            .iter()
            .filter(|f| !f.decision.is_cleanable(smart, force))
            .collect();

        let to_clean_size: u64 = cleanable.iter().map(|f| f.size).sum();
        let skipped_size: u64 = skipped.iter().map(|f| f.size).sum();

        let mode = if force {
            "force"
        } else if smart {
            "smart"
        } else {
            "safe"
        };

        println!();
        println!("DRY RUN");
        println!("───────");
        println!(
            "  Mode: {mode} ({})",
            if force {
                "★★★★★ + ★★★★ + ★★★"
            } else if smart {
                "★★★★★ + ★★★★"
            } else {
                "★★★★★ only"
            }
        );

        // Space accounting: safe vs review vs total
        let owned_cleanable: Vec<rules::ClassifiedFile> =
            cleanable.iter().map(|f| (*f).clone()).collect();
        let cs = confidence::ConfidenceSummary::from_files(&owned_cleanable);

        let safe_size: u64 = owned_cleanable
            .iter()
            .filter(|f| matches!(confidence::confidence(f), confidence::Tier::Maximum))
            .map(|f| f.size)
            .sum();
        let review_size: u64 = owned_cleanable
            .iter()
            .filter(|f| {
                matches!(
                    confidence::confidence(f),
                    confidence::Tier::High | confidence::Tier::Moderate
                )
            })
            .map(|f| f.size)
            .sum();

        println!(
            "  Safe (★★★★★):  {} files ({})",
            cs.maximum,
            simulator::human_size(safe_size)
        );
        println!(
            "  Review (★★★+): {} files ({})",
            cs.high + cs.moderate,
            simulator::human_size(review_size)
        );
        println!(
            "  Total:         {} files ({})",
            cleanable.len(),
            simulator::human_size(to_clean_size)
        );
        println!(
            "  Would skip:    {} files ({})",
            skipped.len(),
            simulator::human_size(skipped_size)
        );

        // Domain breakdown (summary-first)
        let domains = domain::summarize(&owned_cleanable);
        if !domains.is_empty() {
            println!("\n  WOULD CLEAN BY DOMAIN\n");
            for d in domains.iter().take(10) {
                // v6.4.0: Use dominant decision + risk_score to compute tier.
                // Pure risk_score mapping ignores the bridge override that
                // changes toolchain files from Safe to LowRisk.
                // LOWRISK/MIXED-dominant domains (e.g. toolchains) → ★★★★ High.
                let tier = if d.dominant_decision == "SAFE" {
                    confidence::Tier::Maximum
                } else if d.dominant_decision == "BLOCKED" {
                    confidence::Tier::Protected
                } else if d.dominant_decision == "LOWRISK" || d.dominant_decision == "MIXED" {
                    confidence::Tier::High // ★★★★ — requires --smart
                } else if d.risk_score < 0.15 {
                    confidence::Tier::Maximum
                } else if d.risk_score < 0.35 {
                    confidence::Tier::High
                } else {
                    confidence::Tier::Moderate
                };
                println!(
                    "  {} {:<35} {:>5} files  {}",
                    tier.stars(),
                    d.domain,
                    d.file_count as i64,
                    simulator::human_size(d.total_size)
                );
            }
            if domains.len() > 10 {
                println!("  ... and {} more domains", domains.len() - 10);
            }
        }

        // Top contributors (v6.2.1)
        let top = top_contributors(&owned_cleanable, 8);
        if !top.is_empty() {
            println!("\n  TOP CONTRIBUTORS\n");
            for (name, count, size) in &top {
                println!(
                    "  {:<40} {:>4} files  {}",
                    name,
                    count,
                    simulator::human_size(*size)
                );
            }
        }

        // File list only with --verbose
        if verbose && !cleanable.is_empty() {
            println!("\n  FILES\n");
            for f in cleanable.iter().take(50) {
                let tier = confidence::confidence(f);
                println!(
                    "  {} {}  {}",
                    tier.stars(),
                    simulator::human_size(f.size),
                    f.path
                );
            }
            if cleanable.len() > 50 {
                println!("  ... and {} more files", cleanable.len() - 50);
            }
        } else if !cleanable.is_empty() && !verbose {
            println!("\n  (use --verbose for file list)");
        }
        return;
    }

    // Actual clean
    let mut snap = snapshot::Snapshot::new();
    for f in &classified {
        snap.add(&f.path, f.size, None);
    }
    let _snap_path = snap.save().unwrap_or_default();
    let snap_id = chrono_now();
    let report = cleaner::clean(&classified, smart, force);

    if json {
        let out = serde_json::json!({
            "snapshot_id": snap_id,
            "files_removed": report.files_removed,
            "bytes_freed": report.bytes_freed,
            "files_skipped": report.files_skipped,
            "bytes_skipped": report.bytes_skipped,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }

    println!();
    println!("\nCLEAN COMPLETE");
    println!("──────────────");
    println!(
        "  Removed:  {} files ({})",
        report.files_removed,
        simulator::human_size(report.bytes_freed)
    );
    println!(
        "  Skipped:  {} files ({})",
        report.files_skipped,
        simulator::human_size(report.bytes_skipped)
    );
    if !report.errors.is_empty() {
        println!(
            "║  Errors:   {:>5}                                   ║",
            report.errors.len()
        );
    }
    println!("  Snapshot:  {}", snap.id);
    println!("  Undo:      zacxiom undo {}", snap.id);

    if !report.errors.is_empty() {
        println!("\n  Errors:");
        for e in &report.errors {
            println!("    {} → {}", e.path, e.error);
        }
    }
}

fn run_explain(path: &str) {
    // v6.2.4: fixed — distinguish file vs directory, scan correctly
    let target = PathBuf::from(path);
    if !target.exists() {
        eprintln!("No such path: {path}");
        std::process::exit(1);
    }

    let ctx = RunContext::new("dev");

    if target.is_file() {
        // Single file — create a scan entry directly, never scan parent dir
        let size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
        let entry = scanner::ScanEntry {
            path: target.clone(),
            size,
        };
        let entries = vec![entry];
        let threads = 1;
        let classified = classify(entries, &ctx, threads);
        let exp = explain::explain_path(path, &classified);
        let mut eng = crate::engine::classify(&target);
        boost_confidence_from_discovery(&mut eng);
        println!("{}", explain::render_card(&exp, Some(&eng)));
        return;
    }

    // Directory — scan only that directory, not parent; use sufficient depth
    let roots = vec![target];
    let entries = scanner::scan(&roots, 8, 1, true);
    let threads = optimal_threads(entries.len());
    let classified = classify(entries, &ctx, threads);

    let exp = explain::explain_path(path, &classified);
    let mut eng = crate::engine::classify(&PathBuf::from(path));
    boost_confidence_from_discovery(&mut eng);
    println!("{}", explain::render_card(&exp, Some(&eng)));
}

/// v8.0: Boost confidence when project ownership is discovered.
fn boost_confidence_from_discovery(eng: &mut crate::engine::ClassificationResult) {
    if let Some(project) = discovery::find_project_for_path(&eng.path) {
        // Only boost if not already at max
        if eng.confidence_score < 95 {
            eng.confidence_score = (eng.confidence_score + 10).min(99);
        }
        let reason = format!(
            "✓ Project ownership discovered: {} ({})",
            project.name,
            project.ecosystem.display()
        );
        if !eng.confidence_reasons.contains(&reason) {
            eng.confidence_reasons.push(reason);
        }
    }
}

/// v6.3.2: Unknown domain intelligence — what's in the Unknown bucket?
fn run_inspect_unknown(paths: Vec<String>, depth: usize, json: bool, verbose: bool) {
    let ctx = RunContext::new("dev");
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    let threads = optimal_threads(entries.len());
    let classified = classify(entries, &ctx, threads);

    let breakdown = inspect::analyze(&classified);

    if json {
        println!("{}", inspect::render_json(&breakdown));
        return;
    }

    println!("{}", inspect::render_report(&breakdown));

    if verbose {
        // Show near-miss: files that have engine_category but low confidence
        let near_miss: Vec<_> = classified
            .iter()
            .filter(|f| {
                !f.engine_category.is_empty()
                    && f.engine_category != "Unknown"
                    && f.engine_confidence < 30
            })
            .take(20)
            .collect();
        if !near_miss.is_empty() {
            println!(
                "\nNEAR MISS (classified but low confidence)\n{}",
                "─".repeat(40)
            );
            for f in &near_miss {
                println!(
                    "  {}  confidence: {}%  → {}",
                    f.engine_category, f.engine_confidence, f.path
                );
            }
        }
    }
}

fn run_undo(id: Option<String>) {
    let snap_id = match id {
        Some(ref i) => i.clone(),
        None => {
            let all = snapshot::Snapshot::list_all();
            if all.is_empty() {
                eprintln!("No snapshots found. Nothing to undo.");
                std::process::exit(1);
            }
            all.last().unwrap().clone()
        }
    };

    println!("Restoring from snapshot: {snap_id}");
    match snapshot::Snapshot::load(&snap_id) {
        Ok(snap) => match snap.restore() {
            Ok(n) => println!("Restored {n} files."),
            Err(e) => eprintln!("Restore error: {e}"),
        },
        Err(e) => eprintln!("Failed to load snapshot: {e}"),
    }
}

fn run_status() {
    let health = profiles::detect_health();
    let hist = history::History::load();
    let snaps = snapshot::Snapshot::list_all();
    let policy = policy::Policy::load();
    let mem = memory::ContextMemory::load();
    let safe = safety::system_health_check();

    println!("────────────");
    println!("  ZACXIOM v{} STATUS", env!("CARGO_PKG_VERSION"));
    println!("────────────");
    println!("  Health    : {:?}", health);
    println!("  History   : {} records", hist.records.len());
    println!("  Snapshots : {} available", snaps.len());
    println!(
        "  Memory    : {} sessions, {} trusted, {} flagged",
        mem.sessions,
        mem.trusted_paths.len(),
        mem.flagged_paths.len()
    );
    println!(
        "  Stability : {}",
        if mem.is_stabilized() {
            "stabilized"
        } else {
            "learning"
        }
    );
    if !policy.protected_paths.is_empty() {
        println!(
            "  Policy    : {} user-protected paths",
            policy.protected_paths.len()
        );
    }
    if !snaps.is_empty() {
        println!("  Last snap : {}", snaps.last().unwrap());
    }
    println!(
        "  Safety    : {}",
        if safe.passed { "PASS" } else { "FAIL" }
    );
    println!("────────────");
}

fn chrono_now() -> String {
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
fn top_contributors(files: &[rules::ClassifiedFile], limit: usize) -> Vec<(String, usize, u64)> {
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
fn contributor_name(path: &str) -> String {
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

    // Fallback: extract app name from path
    path.split('/')
        .find(|p| p.contains(".cache") || p.contains(".config") || p.contains(".local"))
        .map(|s| {
            let parts: Vec<&str> = s.split('/').collect();
            if parts.len() >= 2 {
                parts[parts.len() - 1].to_string()
            } else {
                s.to_string()
            }
        })
        .unwrap_or_else(|| "Other".into())
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use crate::confidence;
    use crate::rules::{ClassifiedFile, Decision, Ownership};

    /// Build a ClassifiedFile as if it went through the full scan pipeline
    /// for a given path, then check the final decision and tier.
    fn classify_via_pipeline(path: &str) -> (Decision, confidence::Tier, String) {
        let path_buf = PathBuf::from(path);

        // Step 1: Legacy cache classifier
        let domain = cache::classify(&path_buf);

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
                risk_reasons
                    .push("Downloaded artifact: regenerable but expensive to restore".into());
            }
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
        let (decision, _tier, engine_cat) = classify_via_pipeline(
            "/home/user/.cargo/registry/cache/index.crates.io-abc/syn-1.0.crate",
        );

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
        // ~/Downloads should be UserDocument → Moderate → requires --force
        let (decision, _tier, engine_cat) =
            classify_via_pipeline("/home/user/Downloads/installer.iso");

        assert_eq!(engine_cat, "User Document");
        assert_eq!(decision, Decision::Moderate);
        assert!(!decision.is_cleanable(false, false)); // NOT cleanable in safe mode
        assert!(!decision.is_cleanable(true, false)); // NOT cleanable with --smart either
        assert!(decision.is_cleanable(false, true)); // only with --force
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
        let cache_domain = cache::classify(&path_buf);
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
}
