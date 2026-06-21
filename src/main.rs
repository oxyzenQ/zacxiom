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

fn main() {
    let cli = Cli::parse();

    match cli.command {
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
    }
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
        // Report mode: full output
        let sim = simulator::simulate(&classified);
        println!("{}", simulator::format_report(&sim));
    } else {
        // Scan mode: brief
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
