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

fn main() {
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

        Command::Undo { id } => commands::run_undo(id),
        Command::Status => commands::run_status(),
        Command::Plan { path } => commands::run_plan(path),
        Command::InspectUnknown {
            paths,
            depth,
            json,
            verbose,
        } => commands::run_inspect_unknown(paths, depth, json, verbose),
        Command::CheckUpdate => commands::check_update(),
        Command::ExplainConfidence { path } => commands::run_explain_confidence(path),
        Command::ExplainRisk { path } => commands::run_explain_risk(path),
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
