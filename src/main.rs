// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.

mod cache;
mod cleaner;
mod cli;
mod ownership;
mod risk;
mod rules;
mod scanner;
mod simulator;

use clap::Parser;
use cli::{Cli, Command};
use std::path::PathBuf;
use std::process;

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

fn main() {
    let cli = Cli::parse();

    // Handle --version / -V before any command
    if cli.version {
        print_version();
        return;
    }

    // Handle --check-update
    if cli.check_update {
        check_update();
        return;
    }

    let command = cli.command.unwrap_or_else(|| {
        eprintln!("No command specified. Use --help for usage.");
        process::exit(1);
    });

    match command {
        Command::Scan {
            paths,
            depth,
            min_size,
            json,
        } => run_scan(paths, depth, min_size, json, false),

        Command::Report { paths, depth, json } => run_scan(paths, depth, 1, json, true),

        Command::Simulate { paths, depth, json } => run_simulate(paths, depth, json),

        Command::Clean {
            paths,
            depth,
            smart,
            force,
            json,
        } => run_clean(paths, depth, smart, force, json),

        Command::CheckUpdate => check_update(),
    }
}

/// Print version in masterclass format.
fn print_version() {
    let git_hash = option_env!("ZACXIOM_GIT_HASH").unwrap_or("unknown");
    println!("zacxiom -V/--version");
    println!("Version: v{}", env!("CARGO_PKG_VERSION"));
    println!("Build: {} ({})", BUILD_TARGET, git_hash);
    println!("Copyright: (c) 2026 rezky_nightky (oxyzenQ)");
    println!("License: GPL-3.0");
    println!("Source: https://github.com/oxyzenQ/zacxiom");
}

/// Check latest upstream release via GitHub API.
fn check_update() {
    println!("Checking for updates...");

    let current = env!("CARGO_PKG_VERSION");
    println!("Current: v{}", current);

    match fetch_latest_release() {
        Ok(latest) => {
            if latest != current {
                println!("Latest : v{}", latest);
                println!(
                    "Update available: https://github.com/oxyzenQ/zacxiom/releases/tag/v{}",
                    latest
                );
            } else {
                println!("zacxiom is up to date (v{}).", current);
            }
        }
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
        }
    }
}

/// Fetch latest release tag from GitHub API.
fn fetch_latest_release() -> Result<String, String> {
    let url = "https://api.github.com/repos/oxyzenQ/zacxiom/releases/latest";
    let response = ureq::get(url)
        .header("User-Agent", "zacxiom-check-update")
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Read error: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Parse error: {e}"))?;

    json["tag_name"]
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
        .ok_or_else(|| "No tag_name in response".to_string())
}

/// Resolve scan roots: use provided paths or defaults.
fn resolve_roots(paths: Vec<String>) -> Vec<PathBuf> {
    if paths.is_empty() {
        scanner::default_scan_roots()
    } else {
        paths.into_iter().map(PathBuf::from).collect()
    }
}

/// Pipeline: scan → classify → ownership → score → output
fn run_scan(paths: Vec<String>, depth: usize, min_size: u64, json: bool, verbose: bool) {
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, min_size, true);

    let classified: Vec<rules::ClassifiedFile> = entries
        .into_iter()
        .map(|e| {
            let domain = cache::classify(&e.path);
            let ownership = ownership::detect(&e.path);
            risk::score(&e.path.to_string_lossy(), e.size, domain, ownership)
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&classified).unwrap());
    } else if verbose {
        let sim = simulator::simulate(&classified);
        println!("{}", simulator::format_report(&sim));
    } else {
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

/// Pipeline: scan → classify → ownership → score → simulate → report
fn run_simulate(paths: Vec<String>, depth: usize, json: bool) {
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);

    let classified: Vec<rules::ClassifiedFile> = entries
        .into_iter()
        .map(|e| {
            let domain = cache::classify(&e.path);
            let ownership = ownership::detect(&e.path);
            risk::score(&e.path.to_string_lossy(), e.size, domain, ownership)
        })
        .collect();

    let report = simulator::simulate(&classified);

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!("{}", simulator::format_report(&report));
    }
}

/// Pipeline: scan → classify → ownership → score → simulate → clean
fn run_clean(paths: Vec<String>, depth: usize, smart: bool, force: bool, json: bool) {
    let roots = resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);

    let classified: Vec<rules::ClassifiedFile> = entries
        .into_iter()
        .map(|e| {
            let domain = cache::classify(&e.path);
            let ownership = ownership::detect(&e.path);
            risk::score(&e.path.to_string_lossy(), e.size, domain, ownership)
        })
        .collect();

    // H5: simulation MUST run before clean
    let sim_report = simulator::simulate(&classified);
    println!("{}", simulator::format_report(&sim_report));

    // H6: force mode gating
    if force {
        use std::io::{self, Write};
        print!("\n⚠️  --force mode: Type YES to proceed: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if input.trim() != "YES" {
            println!("Clean aborted.");
            process::exit(1);
        }
    }

    let clean_report = cleaner::clean(&classified, smart, force);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "files_removed": clean_report.files_removed,
                "bytes_freed": clean_report.bytes_freed,
                "files_skipped": clean_report.files_skipped,
                "bytes_skipped": clean_report.bytes_skipped,
                "errors": clean_report.errors.iter().map(|e| serde_json::json!({"path": e.path, "error": e.error})).collect::<Vec<_>>(),
            }))
            .unwrap()
        );
    } else {
        println!("{}", cleaner::format_clean_report(&clean_report));
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
