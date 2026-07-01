// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom inspect-unknown` — unknown domain intelligence command.

use crate::config::Config;
use crate::exclude::ExcludeFilter;
use crate::inspect;
use crate::pipeline::{self, RunContext};
use crate::scanner;

pub fn run_inspect_unknown(
    paths: Vec<String>,
    depth: usize,
    json: bool,
    verbose: bool,
    cfg: &Config,
) {
    let ctx = RunContext::new("dev");
    let roots = pipeline::resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true, &ExcludeFilter::empty());
    let threads = pipeline::optimal_threads_with_config(entries.len(), cfg.scan.max_threads);
    let classified = pipeline::classify(
        entries,
        &ctx,
        threads,
        cfg,
        &crate::scan_cache::ScanCache::new(),
    );

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
