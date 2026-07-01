// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine v12.0.0
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.
#![allow(dead_code)]

mod advisor;
mod audit;
mod cache;
mod cleaner;
mod cli;
mod color;
mod commands;
mod confidence;
mod config;
mod decision;
mod dependency;
mod discovery;
mod display;
mod domain;
mod ecosystem;
mod engine;
mod environment;
mod errors;
mod evidence;
mod exclude;
mod execution_order;
mod explain;
mod history;
mod ignorefile;
mod impact;
mod inspect;
mod memory;
mod ownership;
mod parallel;
mod pipeline;
mod planner;
mod policy;
mod procfs;
mod profiles;
mod progress;
mod risk;
mod rules;
mod safety;
mod scan_cache;
mod scanner;
mod simulator;
mod snapshot;
mod summary;
mod workspace;

use clap::Parser;
use cli::{Cli, Command};
use std::sync::Once;

static PANIC_HOOK: Once = Once::new();

/// Install a global panic hook that prints location, message, and full backtrace.
/// Safe to call multiple times — only the first call takes effect.
pub fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let loc = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "<unknown>".to_string());
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".to_string());
            eprintln!("\n━━━ PANIC ━━━");
            eprintln!("  location : {loc}");
            eprintln!("  message  : {msg}");
            eprintln!("━━━━━━━━━━━━━━");
            let bt = std::backtrace::Backtrace::force_capture();
            eprintln!("{bt}");
        }));
    });
}

fn main() {
    install_panic_hook();
    color::init();
    let cli = Cli::parse();
    // v13.2: Enable colorblind mode if --colorblind flag is set
    if cli.colorblind {
        color::set_colorblind(true);
    }
    // v13.3: Quiet mode — suppress progress output for cron/scripts
    if cli.quiet {
        std::env::set_var("ZACXIOM_QUIET", "1");
    }
    if cli.version {
        pipeline::print_version();
        return;
    }
    if cli.check_update {
        commands::check_update();
        return;
    }

    // v13: --testconf validates the config file and exits.
    if cli.testconf {
        run_testconf();
        return;
    }

    // v13: Load config ONCE. Hard error on malformed config (user explicitly wrote it).
    // This prevents typos from silently weakening safety.
    let cfg = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\u{2501}\u{2501}\u{2501} CONFIG ERROR \u{2501}\u{2501}\u{2501}");
            eprintln!(
                "  Your config file at {} is invalid:",
                config::config_path().display()
            );
            eprintln!();
            eprintln!("  {e}");
            eprintln!();
            eprintln!("  Fix the issue above, then run `zacxiom --testconf` to verify.");
            eprintln!(
                "  Or remove the config to use safe defaults: rm {}",
                config::config_path().display()
            );
            std::process::exit(2);
        }
    };

    let command = cli.command.unwrap_or_else(|| {
        eprintln!("No command specified. Use --help for usage.");
        std::process::exit(1);
    });

    // v13.1: --no-cache flag disables incremental scan cache
    let use_cache = !cli.no_cache;

    // v13.1: Auto-prune old snapshots in background (non-blocking)
    snapshot::auto_prune_async(cfg.snapshot.auto_prune_days);

    match command {
        Command::Scan {
            paths,
            positional_paths,
            depth,
            min_size,
            profile,
            json,
            exclude,
            suggest,
        } => {
            let mut all_paths = paths;
            all_paths.extend(positional_paths);
            commands::run_scan(
                all_paths, depth, min_size, json, false, &profile, &cfg, &exclude, use_cache,
                suggest,
            )
        }

        Command::Report {
            paths,
            positional_paths,
            depth,
            profile,
            json,
            exclude,
        } => {
            let mut all_paths = paths;
            all_paths.extend(positional_paths);
            commands::run_scan(
                all_paths, depth, 1, json, true, &profile, &cfg, &exclude, use_cache, false,
            )
        }

        Command::Simulate {
            paths,
            positional_paths,
            depth,
            profile,
            json,
            verbose,
            exclude,
        } => {
            let mut all_paths = paths;
            all_paths.extend(positional_paths);
            commands::run_simulate(all_paths, depth, json, verbose, &profile, &cfg, &exclude)
        }

        Command::Clean {
            paths,
            positional_paths,
            depth,
            profile,
            smart,
            force,
            dry_run,
            verbose,
            json,
            exclude,
            include,
            fail_fast,
            diff,
            yes,
        } => {
            let mut all_paths = paths;
            all_paths.extend(positional_paths);
            commands::run_clean(
                all_paths, depth, smart, force, dry_run, verbose, json, &profile, &cfg, &exclude,
                yes, fail_fast, &include, diff,
            )
        }

        Command::Explain { path } => {
            if path.is_empty() {
                eprintln!("Usage: zacxiom explain <path>");
                std::process::exit(1);
            }
            commands::run_explain(&path, &cfg);
        }

        Command::Undo { id, list } => commands::run_undo(id, list),
        Command::Status { golden } => commands::run_status(golden),
        Command::Doctor { golden } => {
            if !commands::run_doctor(golden) {
                std::process::exit(1);
            }
        }
        Command::Plan { path } => {
            let target = path.unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .unwrap_or_else(|| "/tmp".into())
                    .to_string_lossy()
                    .to_string()
            });
            commands::run_plan(target);
        }
        Command::InspectUnknown {
            paths,
            depth,
            json,
            verbose,
        } => commands::run_inspect_unknown(paths, depth, json, verbose, &cfg),
        Command::CheckUpdate => commands::check_update(),
        Command::ExplainConfidence { path } => commands::run_explain_confidence(path),
        Command::ExplainRisk { path } => commands::run_explain_risk(path),

        Command::Snapshot { action } => {
            let action = action.unwrap_or_else(|| {
                eprintln!("Usage: zacxiom snapshot <list|delete|prune|purge>");
                std::process::exit(1);
            });
            match action {
                cli::SnapshotAction::List { json } => commands::run_snapshot_list(json),
                cli::SnapshotAction::Delete { id, force } => {
                    commands::run_snapshot_delete(&id, force)
                }
                cli::SnapshotAction::Prune { keep, older_than } => match (keep, older_than) {
                    (Some(n), None) => commands::run_snapshot_prune_keep(n),
                    (None, Some(age)) => commands::run_snapshot_prune_older_than(&age),
                    (None, None) => {
                        eprintln!("Use --keep N or --older-than TIMESPAN (e.g. 30d)");
                        std::process::exit(1);
                    }
                    (Some(_), Some(_)) => {
                        eprintln!("Use --keep OR --older-than, not both");
                        std::process::exit(1);
                    }
                },
                cli::SnapshotAction::Purge { confirm } => {
                    commands::run_snapshot_purge(&confirm.unwrap_or_default());
                }
                cli::SnapshotAction::Verify => {
                    let (total, valid, corrupted) = snapshot::verify_all_snapshots();
                    println!("━━━ SNAPSHOT INTEGRITY CHECK ━━━");
                    println!("  Total:     {total}");
                    println!("  Valid:     {valid}");
                    println!("  Corrupted: {corrupted}");
                    if corrupted > 0 {
                        println!();
                        println!("  ⚠ {corrupted} corrupted snapshot(s) detected.");
                        println!("    Use 'zacxiom snapshot delete <id>' to remove them.");
                        std::process::exit(1);
                    } else if total == 0 {
                        println!();
                        println!("  No snapshots found. Nothing to verify.");
                    } else {
                        println!();
                        println!("  ✅ All snapshots are valid.");
                    }
                }
            }
        }

        Command::Config { action } => match action {
            cli::ConfigAction::Init => commands::run_config_init(),
            cli::ConfigAction::Show => commands::run_config_show(&cfg),
            cli::ConfigAction::Path => commands::run_config_path(),
            cli::ConfigAction::Testconf => run_testconf(),
        },

        Command::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = cli::Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
        }
    }
}

/// Run `--testconf`: validate config, print report, exit 0 (ok) or 1 (error).
fn run_testconf() {
    let report = config::validate_for_testconf();
    println!("\u{2501}\u{2501}\u{2501} ZACXIOM CONFIG CHECK \u{2501}\u{2501}\u{2501}");
    println!("  File: {}", report.file_path.display());
    if !report.file_exists {
        println!("  Status: NOT FOUND (using safe defaults)");
        println!();
        println!("  No config file is required. Zacxiom uses safe defaults.");
        println!("  To create one with recommended settings, run:");
        println!("    zacxiom config init");
        std::process::exit(0);
    }
    println!("  Status: FOUND");
    println!();

    if !report.warnings.is_empty() {
        println!("\u{26a0}\u{fe0f}  WARNINGS (unknown keys \u{2014} will be ignored):",);
        for w in &report.warnings {
            println!("    {w}");
        }
        println!();
    }

    if report.is_ok() {
        println!("\u{2714}\u{fe0f}  Config is valid.");
        println!();
        println!("  Effective configuration:");
        println!("    [scan]");
        println!(
            "      exclude           : {} entries",
            report.config.scan.exclude.len()
        );
        println!(
            "      exclude_patterns  : {} entries",
            report.config.scan.exclude_patterns.len()
        );
        println!(
            "      min_size          : {} bytes",
            report.config.scan.min_size
        );
        println!(
            "      max_threads       : {} {}",
            report.config.scan.max_threads,
            if report.config.scan.max_threads == 0 {
                "(auto)"
            } else {
                "(manual)"
            }
        );
        println!(
            "      warn_user_dirs    : {}",
            report.config.scan.warn_user_dirs
        );
        println!("    [rules_exclude]");
        println!(
            "      exclude           : {} patterns",
            report.config.rules_exclude.exclude.len()
        );
        println!("    [clean]");
        println!(
            "      require_confirmation: {}",
            report.config.clean.require_confirmation
        );
        println!(
            "      default_mode        : {}",
            report.config.clean.default_mode
        );
        println!(
            "      protect_extensions  : {} entries",
            report.config.clean.protect_extensions.len()
        );
        println!(
            "      protect_patterns    : {} entries",
            report.config.clean.protect_patterns.len()
        );
        println!(
            "      max_auto_clean_size : {} bytes",
            report.config.clean.max_auto_clean_size
        );
        println!(
            "      first_run_dry_run   : {}",
            report.config.clean.first_run_dry_run
        );
        println!("    [snapshot]");
        println!("      dir              : {}", report.config.snapshot.dir);
        println!(
            "      auto_prune_days  : {}",
            report.config.snapshot.auto_prune_days
        );
        println!("    [trash]");
        println!(
            "      verify_checksum  : {}",
            report.config.trash.verify_checksum
        );
        std::process::exit(0);
    } else {
        println!("\u{274c}  CONFIG INVALID \u{2014} zacxiom will refuse to run until fixed.");
        println!();
        println!("  Errors:");
        for e in &report.errors {
            println!("    \u{2022} {e}");
            println!();
        }
        println!("  To fix:");
        println!("    1. Edit the file: nano {}", report.file_path.display());
        println!("    2. Re-run this check: zacxiom --testconf");
        println!("    3. Or reset to defaults: zacxiom config init (after deleting the file)");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
