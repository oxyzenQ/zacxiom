// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine v3
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.
//! v5.2: Decision Interface — domain summaries, real risk engine, scan vs simulate.

mod cache;
mod cleaner;
mod cli;
mod display;
mod domain;
mod errors;
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
            json,
        } => run_clean(paths, depth, smart, force, json, &profile),

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
    println!("Checking for updates...");
    let cur = env!("CARGO_PKG_VERSION");
    println!("Current version: v{cur}");

    match fetch_latest_release() {
        Ok(latest) if latest != cur => {
            println!("Latest version : v{latest}");
            println!("Update: https://github.com/oxyzenQ/zacxiom/releases/tag/v{latest}");
        }
        Ok(_) => println!("zacxiom is up to date (v{cur})."),
        Err(e) => {
            let user_msg = if e.contains("403") || e.contains("rate") {
                "Update check unavailable (GitHub API rate limited).\nRetry later or check: https://github.com/oxyzenQ/zacxiom/releases"
            } else if e.contains("404") {
                "No releases found yet."
            } else {
                "Update check unavailable (network issue).\nVerify at: https://github.com/oxyzenQ/zacxiom"
            };
            println!("{user_msg}");
        }
    }
}

fn fetch_latest_release() -> Result<String, String> {
    let resp = ureq::get("https://api.github.com/repos/oxyzenQ/zacxiom/releases/latest")
        .header("User-Agent", "zacxiom-check-update")
        .call()
        .map_err(|e| format!("HTTP: {e}"))?;
    let body = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Read: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("Parse: {e}"))?;
    json["tag_name"]
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
        .ok_or_else(|| "No tag_name".into())
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
    prog.start_phase(progress::Phase::Scan);

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

    // SCAN = analytical: domain summary + decision summary
    let ds = summary::DecisionSummary::from_files(&classified, ctx.open_files.len());
    println!("\n{}", display::render_decision_summary(&ds));

    let domains = domain::summarize(&classified);
    println!("{}", display::render_domain_summary(&domains));

    if verbose {
        println!("{}", display::render_table(&classified, "FILE DETAIL"));
    }
}

fn run_simulate(paths: Vec<String>, depth: usize, json: bool, profile: &str) {
    let mut prog = progress::Progress::new(json);
    prog.start_phase(progress::Phase::Scan);

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

    // SIMULATE = operational: what would happen if I clean now
    let ds = summary::DecisionSummary::from_files(&classified, ctx.open_files.len());
    println!("\n{}", display::render_decision_summary(&ds));

    // Show what's safe + what's blocked with action labels
    println!(
        "{}",
        display::render_simulation(&classified, "DRY RUN — What Would Happen")
    );

    // Insight footer
    let (safe, low, mod_, high, prot) = count_decisions(&classified);
    let insight_ctx = display::InsightContext {
        total: classified.len(),
        safe,
        low_risk: low,
        moderate: mod_,
        high_risk: high,
        protected: prot,
        total_size: classified.iter().map(|f| f.size).sum(),
        open_files: ctx.open_files.len(),
    };
    println!("{}", display::render_insights(&insight_ctx));

    // Rollback availability
    let snaps = snapshot::Snapshot::list_all();
    if !snaps.is_empty() {
        println!(
            "  ⚡ Rollback: {} snapshot(s) available ({})",
            snaps.len(),
            snaps.last().unwrap()
        );
        println!("     Use 'zacxiom undo' to restore.\n");
    }
}

fn run_clean(
    paths: Vec<String>,
    depth: usize,
    smart: bool,
    force: bool,
    json: bool,
    profile: &str,
) {
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    let classified = classify(entries, &ctx);

    let _sim = simulator::simulate(&classified, &ctx.health, &ctx.profile);
    if !json {
        let ds = summary::DecisionSummary::from_files(&classified, ctx.open_files.len());
        println!("\n{}", display::render_decision_summary(&ds));
        println!(
            "{}",
            display::render_simulation(&classified, "BEFORE CLEAN")
        );
        let (safe, low, mod_, high, prot) = count_decisions(&classified);
        let insight_ctx = display::InsightContext {
            total: classified.len(),
            safe,
            low_risk: low,
            moderate: mod_,
            high_risk: high,
            protected: prot,
            total_size: classified.iter().map(|f| f.size).sum(),
            open_files: ctx.open_files.len(),
        };
        println!("{}", display::render_insights(&insight_ctx));
    }

    // v5: Safety lock validation
    let check = safety::validate_clean(&classified, smart, force, true);
    if !check.passed {
        eprintln!("SAFETY LOCK: Clean blocked.");
        for v in &check.violations {
            eprintln!("  ❌ {v}");
        }
        std::process::exit(1);
    }

    if force {
        use std::io::{self, Write};
        print!("\n⚠️  --force mode: Type YES to proceed: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if input.trim() != "YES" {
            println!("Clean aborted.");
            std::process::exit(1);
        }
    }

    // v4: Snapshot before clean for undo support
    let mut snap = snapshot::Snapshot::new();
    let cleaned_paths: Vec<String> = classified
        .iter()
        .filter(|f| f.decision.is_cleanable(smart, force))
        .map(|f| f.path.clone())
        .collect();
    for f in &classified {
        if f.decision.is_cleanable(smart, force) {
            snap.add(&f.path, f.size, None);
        }
    }
    let _ = snap.save();

    let report = cleaner::clean(&classified, smart, force);

    // v5: Record to context memory
    let mut mem = memory::ContextMemory::load();
    mem.record_clean(&cleaned_paths);

    let mut hist = history::History::load();
    hist.add(history::CleanRecord {
        timestamp: chrono_now(),
        version: env!("CARGO_PKG_VERSION").into(),
        action: if force {
            "clean --force".into()
        } else if smart {
            "clean --smart".into()
        } else {
            "clean".into()
        },
        files_removed: report.files_removed,
        bytes_freed: report.bytes_freed,
        files_skipped: report.files_skipped,
        paths: classified.iter().map(|f| f.path.clone()).collect(),
    });

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "files_removed": report.files_removed,
                "bytes_freed": report.bytes_freed,
                "files_skipped": report.files_skipped,
                "bytes_skipped": report.bytes_skipped,
                "errors": report.errors.iter().map(|e| {
                    let kind = errors::ErrorKind::from_io_error(
                        &std::io::Error::other(e.error.clone())
                    );
                    serde_json::json!({"path": e.path, "kind": kind.label(), "detail": e.error})
                }).collect::<Vec<_>>(),
            }))
            .unwrap()
        );
    } else {
        let clean_msg = format!(
            "CLEAN COMPLETE — {} files freed ({})",
            report.files_removed,
            display::human_size(report.bytes_freed)
        );
        let mut err_summary = errors::ErrorSummary::default();
        for e in &report.errors {
            let kind = errors::ErrorKind::from_io_error(&std::io::Error::other(e.error.clone()));
            match kind {
                errors::ErrorKind::PermissionDenied => err_summary.permission_denied += 1,
                errors::ErrorKind::InUse => err_summary.in_use += 1,
                errors::ErrorKind::SystemFile => err_summary.system_protected += 1,
                errors::ErrorKind::LockedProcess => err_summary.locked += 1,
                errors::ErrorKind::NotFound => err_summary.not_found += 1,
                _ => err_summary.unknown += 1,
            }
        }
        println!("\n┌{:─^78}┐", "");
        println!("│ {:^76} │", clean_msg);
        if report.files_skipped > 0 {
            println!(
                "│ {:<76} │",
                format!(
                    "Skipped: {} files ({})",
                    report.files_skipped,
                    display::human_size(report.bytes_skipped)
                )
            );
        }
        if !err_summary.is_empty() {
            let summary_lines = err_summary.format_summary();
            for line in summary_lines.lines() {
                println!("│ {:<76} │", line.trim());
            }
        }
        println!("└{:─^78}┘", "");
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

    println!("═══════════════════════════════════");
    println!("  ZACXIOM STATUS");
    println!("═══════════════════════════════════");
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
    println!("═══════════════════════════════════");
}

fn count_decisions(files: &[rules::ClassifiedFile]) -> (usize, usize, usize, usize, usize) {
    let (mut safe, mut low, mut mod_, mut high, mut prot) = (0, 0, 0, 0, 0);
    for f in files {
        match f.decision {
            rules::Decision::Safe => safe += 1,
            rules::Decision::LowRisk => low += 1,
            rules::Decision::Moderate => mod_ += 1,
            rules::Decision::HighRisk => high += 1,
            rules::Decision::Protected => prot += 1,
        }
    }
    (safe, low, mod_, high, prot)
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
