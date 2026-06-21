// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine v3
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.
//! v4: Safety autonomy — snapshots, undo, policy engine.

mod cache;
mod cleaner;
mod cli;
mod history;
mod ownership;
mod policy;
mod procfs;
mod profiles;
mod risk;
mod rules;
mod scanner;
mod simulator;
mod snapshot;

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
    println!("Current: v{cur}");
    match fetch_latest_release() {
        Ok(latest) if latest != cur => {
            println!("Latest : v{latest}");
            println!("Update: https://github.com/oxyzenQ/zacxiom/releases/tag/v{latest}");
        }
        Ok(_) => println!("zacxiom is up to date (v{cur})."),
        Err(e) => eprintln!("Failed to check for updates: {e}"),
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
            risk::score_v2(
                &e.path.to_string_lossy(),
                e.size,
                d,
                o,
                Some(&ctx.open_files),
                Some(&ctx.history_cleaned),
            )
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
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, min_size, true);
    let classified = classify(entries, &ctx);
    let health_str = format!("{:?}", ctx.health);
    let profile_str = format!("{:?}", ctx.profile);

    if json {
        let out = serde_json::json!({
            "health": health_str,
            "profile": profile_str,
            "files": classified,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else if verbose {
        let sim = simulator::simulate(&classified, &ctx.health, &ctx.profile);
        println!("{}", simulator::format_report(&sim));
    } else {
        println!("Health: {health_str} | Profile: {profile_str}");
        println!("Scanned {} files", classified.len());
        let total: u64 = classified.iter().map(|f| f.size).sum();
        println!("Total size: {}", simulator::human_size(total));
        for f in &classified {
            println!(
                "  {} [{:.2}] → {}",
                f.path,
                f.risk_score,
                format_decision(&f.decision)
            );
        }
    }
}

fn run_simulate(paths: Vec<String>, depth: usize, json: bool, profile: &str) {
    let ctx = RunContext::new(profile);
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    let classified = classify(entries, &ctx);
    let report = simulator::simulate(&classified, &ctx.health, &ctx.profile);

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!("{}", simulator::format_report(&report));
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

    let sim = simulator::simulate(&classified, &ctx.health, &ctx.profile);
    println!("{}", simulator::format_report(&sim));

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
    for f in &classified {
        if f.decision.is_cleanable(smart, force) {
            snap.add(&f.path, f.size, None);
        }
    }
    let _ = snap.save();

    let report = cleaner::clean(&classified, smart, force);

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
                "errors": report.errors.iter().map(|e| serde_json::json!({"path": e.path, "error": e.error})).collect::<Vec<_>>(),
            }))
            .unwrap()
        );
    } else {
        println!("{}", cleaner::format_clean_report(&report));
    }
}

fn format_decision(d: &rules::Decision) -> &'static str {
    match d {
        rules::Decision::Safe => "SAFE",
        rules::Decision::LowRisk => "LOW_RISK",
        rules::Decision::Moderate => "MODERATE",
        rules::Decision::HighRisk => "HIGH_RISK",
        rules::Decision::Protected => "PROTECTED",
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

    println!("═══════════════════════════════════");
    println!("  ZACXIOM STATUS");
    println!("═══════════════════════════════════");
    println!("  Health    : {:?}", health);
    println!("  History   : {} records", hist.records.len());
    println!("  Snapshots : {} available", snaps.len());
    if !policy.protected_paths.is_empty() {
        println!(
            "  Policy    : {} user-protected paths",
            policy.protected_paths.len()
        );
    }
    if !snaps.is_empty() {
        println!("  Last snap : {}", snaps.last().unwrap());
    }
    println!("═══════════════════════════════════");
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
