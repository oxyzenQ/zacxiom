// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom — Filesystem Intelligence Engine v7.0.0
//!
//! Observe → Understand → Decide → Act
//! Safe by default. Explainable by design.
//! v10.0.0: Release hardening — stability, correctness, regression tests.
#![allow(dead_code)]

mod advisor;
mod cache;
mod cleaner;
mod cli;
mod color;
mod commands;
mod confidence;
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
mod execution_order;
mod explain;
mod history;
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
    if cli.version {
        pipeline::print_version();
        return;
    }
    if cli.check_update {
        commands::check_update();
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
        } => commands::run_scan(paths, depth, min_size, json, false, &profile),

        Command::Report {
            paths,
            depth,
            profile,
            json,
        } => commands::run_scan(paths, depth, 1, json, true, &profile),

        Command::Simulate {
            paths,
            depth,
            profile,
            json,
            verbose,
        } => commands::run_simulate(paths, depth, json, verbose, &profile),

        Command::Clean {
            paths,
            depth,
            profile,
            smart,
            force,
            dry_run,
            verbose,
            json,
        } => commands::run_clean(paths, depth, smart, force, dry_run, verbose, json, &profile),

        Command::Explain { path } => {
            if path.is_empty() {
                eprintln!("Usage: zacxiom explain <path>");
                std::process::exit(1);
            }
            commands::run_explain(&path);
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
        } => commands::run_inspect_unknown(paths, depth, json, verbose),
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
            }
        }
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
