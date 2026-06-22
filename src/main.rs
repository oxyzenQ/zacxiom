// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine v6.2.0
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.
//! v6.2: Trust Release — explain, dry-run, confidence tiers, top contributors.
#![allow(dead_code)]

mod cache;
mod cleaner;
mod cli;
mod confidence;
mod display;
mod domain;
mod errors;
mod explain;
mod history;
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
use std::collections::HashSet;
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
        } => run_simulate(paths, depth, json, &profile),

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

fn classify(entries: Vec<scanner::ScanEntry>, ctx: &RunContext) -> Vec<rules::ClassifiedFile> {
    entries
        .into_iter()
        .map(|e| {
            let d = cache::classify(&e.path);
            let o = ownership::detect(&e.path);
            let path_str = e.path.to_string_lossy().to_string();
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
            scored
        })
        .collect()
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
    let classified = classify(entries, &ctx);
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

    if verbose {
        println!("{}", display::render_table(&classified, "FILE DETAIL"));
    }
}

fn run_simulate(paths: Vec<String>, depth: usize, json: bool, profile: &str) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    prog.advance();
    let classified = classify(entries, &ctx);
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

    let _report = simulator::simulate(&classified, &ctx.health, &ctx.profile);
    println!("{}", display::render_simulation(&classified, "SIMULATION"));
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
    let classified = classify(entries, &ctx);
    prog.advance();
    prog.advance();
    prog.done();

    if dry_run {
        // v6.2.0: dry-run mode — preview only
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
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║           DRY RUN — Preview Only (no files deleted)     ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!(
            "║  Mode:         {:<41} ║",
            format!(
                "{mode} ({})",
                if force {
                    "★★★★★ + ★★★★ + ★★★"
                } else if smart {
                    "★★★★★ + ★★★★"
                } else {
                    "★★★★★ only"
                }
            )
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
            "║  Safe (★★★★★):  {:<41} ║",
            format!(
                "{} files ({})",
                cs.maximum,
                simulator::human_size(safe_size)
            )
        );
        println!(
            "║  Review (★★★+):  {:<40} ║",
            format!(
                "{} files ({})",
                cs.high + cs.moderate,
                simulator::human_size(review_size)
            )
        );
        println!(
            "║  Total:         {:>5} files ({:<33}) ║",
            cleanable.len(),
            simulator::human_size(to_clean_size)
        );
        println!(
            "║  Would skip:    {:>5} files ({:<33}) ║",
            skipped.len(),
            simulator::human_size(skipped_size)
        );
        println!("╚══════════════════════════════════════════════════════════╝");

        // Domain breakdown (summary-first)
        let domains = domain::summarize(&owned_cleanable);
        if !domains.is_empty() {
            println!("\n  ── WOULD CLEAN BY DOMAIN ──\n");
            for d in domains.iter().take(10) {
                let tier = if d.risk_score < 0.15 {
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
                    d.file_count,
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
            println!("\n  ── TOP CONTRIBUTORS ──\n");
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
            println!("\n  ── FILES ──\n");
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
            println!("\n  Use --verbose to see individual file list");
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
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║           CLEAN COMPLETE                                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!(
        "║  Removed:  {:>5} files ({:<33}) ║",
        report.files_removed,
        simulator::human_size(report.bytes_freed)
    );
    println!(
        "║  Skipped:  {:>5} files ({:<33}) ║",
        report.files_skipped,
        simulator::human_size(report.bytes_skipped)
    );
    if !report.errors.is_empty() {
        println!(
            "║  Errors:   {:>5}                                   ║",
            report.errors.len()
        );
    }
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Snapshot:  {:<43} ║", snap.id);
    println!("║  Undo:      zacxiom undo {:<31} ║", snap.id);
    println!("╚══════════════════════════════════════════════════════════╝");

    if !report.errors.is_empty() {
        println!("\n  Errors:");
        for e in &report.errors {
            println!("    {} → {}", e.path, e.error);
        }
    }
}

fn run_explain(path: &str) {
    // v6.2.0: explain command — show 5-question trust card
    // Scan the area around the path to find files
    let target = PathBuf::from(path);
    let roots = if target.is_dir() {
        vec![target.clone()]
    } else if let Some(parent) = target.parent() {
        vec![parent.to_path_buf()]
    } else {
        eprintln!("Invalid path: {path}");
        std::process::exit(1);
    };

    let ctx = RunContext::new("dev");
    let entries = scanner::scan(&roots, 1, 1, true);
    let classified = classify(entries, &ctx);

    if classified.is_empty() {
        // Try domain-level explanation
        let domains = domain::summarize(&[]);
        for d in &domains {
            if d.domain.to_lowercase().contains(&path.to_lowercase()) {
                let tier = if d.risk_score < 0.15 {
                    confidence::Tier::Maximum
                } else if d.risk_score < 0.35 {
                    confidence::Tier::High
                } else {
                    confidence::Tier::Moderate
                };
                let exp = explain::explain_domain(&d.domain, d.total_size, tier, d.file_count);
                println!("{}", explain::render_card(&exp));
                return;
            }
        }
        eprintln!("No files found at: {path}");
        std::process::exit(1);
    }

    // File-level explanations for top matches
    for f in classified.iter().take(5) {
        let exp = explain::explain_file(f);
        println!("{}", explain::render_card(&exp));
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

    println!("═══════════════════════════════════════");
    println!("  ZACXIOM v{} STATUS", env!("CARGO_PKG_VERSION"));
    println!("═══════════════════════════════════════");
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
    println!("═══════════════════════════════════════");
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
