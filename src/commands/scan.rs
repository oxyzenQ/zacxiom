// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom scan` — filesystem scan command.

use crate::confidence;
use crate::display;
use crate::domain;
use crate::inspect;
use crate::pipeline::{self, get_open_files, RunContext};
use crate::progress;
use crate::scanner;
use crate::summary;

pub fn run_scan(
    paths: Vec<String>,
    depth: usize,
    min_size: u64,
    json: bool,
    verbose: bool,
    profile: &str,
) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = pipeline::resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, min_size, true);
    prog.advance();
    let threads = pipeline::optimal_threads(entries.len());
    prog.set_threads(threads);
    let classified = pipeline::classify(entries, &ctx, threads);
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

    let ds = summary::DecisionSummary::from_files(&classified, get_open_files().len());
    println!("\n{}", display::render_decision_summary(&ds));

    let domains = domain::summarize(&classified);
    println!("{}", display::render_domain_summary(&domains));

    // v6.2.0: show confidence breakdown
    let cs = confidence::ConfidenceSummary::from_files(&classified);
    println!("{}", display::render_confidence_summary(&cs));

    // v6.3.2: classifier coverage
    let cov = inspect::analyze(&classified);
    println!("{}", inspect::render_coverage(&cov));

    if verbose {
        println!("{}", display::render_table(&classified, "FILE DETAIL"));
    }

    // Show next recommended command
    let cleanable = classified
        .iter()
        .filter(|f| f.decision.is_cleanable(false, false))
        .count();
    if cleanable > 0 {
        println!("\n  💡 Next: zacxiom plan  (read-only preview)");
        println!("         zacxiom clean (safe files only)");
    } else {
        println!("\n  💡 Next: zacxiom clean --smart (includes low-risk files)");
        println!("         zacxiom plan  (see what could be cleaned)");
    }
}
