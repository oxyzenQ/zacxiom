// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom report` / `zacxiom simulate` — dry-run simulation command.

use crate::confidence;
use crate::config::Config;
use crate::display;
use crate::domain;
use crate::inspect;
use crate::pipeline::{self, get_open_files, RunContext};
use crate::progress;
use crate::rules;
use crate::scanner;
use crate::simulator;
use crate::summary;

pub fn run_simulate(
    paths: Vec<String>,
    depth: usize,
    json: bool,
    verbose: bool,
    profile: &str,
    cfg: &Config,
    cli_exclude: &[String],
) {
    let mut prog = progress::Progress::new(json || std::env::var("ZACXIOM_QUIET").is_ok());
    let ctx = RunContext::new(profile);
    let roots = pipeline::resolve_roots(paths);
    let exclude = pipeline::build_exclude_filter(cfg, cli_exclude);
    let entries = scanner::scan(&roots, depth, 1, true, &exclude);
    prog.advance();
    let threads = pipeline::optimal_threads_with_config(entries.len(), cfg.scan.max_threads);
    prog.set_threads(threads);
    let classified = pipeline::classify(entries, &ctx, threads, cfg);
    prog.advance();
    prog.advance();
    prog.done();

    if json {
        let report = simulator::simulate(&classified, &ctx.health, &ctx.profile);
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    let ds = summary::DecisionSummary::from_files(&classified, get_open_files().len());
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
            let top = pipeline::top_contributors(
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
