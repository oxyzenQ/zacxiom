// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom clean` — cleanup execution command.

use crate::cleaner;
use crate::confidence;
use crate::domain;
use crate::pipeline::{self, RunContext};
use crate::progress;
use crate::rules;
use crate::scanner;
use crate::simulator;
use crate::snapshot;

#[allow(clippy::too_many_arguments)]
pub fn run_clean(
    paths: Vec<String>,
    depth: usize,
    smart: bool,
    force: bool,
    dry_run: bool,
    verbose: bool,
    json: bool,
    profile: &str,
) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = pipeline::resolve_roots(paths);
    let entries = scanner::scan(&roots, depth, 1, true);
    prog.advance();
    let threads = pipeline::optimal_threads(entries.len());
    prog.set_threads(threads);
    let classified = pipeline::classify(entries, &ctx, threads);
    prog.advance();
    prog.advance();
    prog.done();

    if dry_run {
        // v6.2.2: clean summary, no box art
        let cleanable: Vec<_> = classified
            .iter()
            .filter(|f| f.decision.is_cleanable(smart, force))
            .collect();
        let skipped: Vec<_> = classified
            .iter()
            .filter(|f| !f.decision.is_cleanable(smart, force))
            .collect();

        let to_clean_size: u64 = cleanable.iter().map(|f| f.size).sum();
        let skipped_size: u64 = skipped.iter().map(|f| f.size).sum();

        let mode = if force {
            "force"
        } else if smart {
            "smart"
        } else {
            "safe"
        };

        println!();
        println!("DRY RUN");
        println!("───────");
        println!(
            "  Mode: {mode} ({})",
            if force {
                "★★★★★ + ★★★★ + ★★★"
            } else if smart {
                "★★★★★ + ★★★★"
            } else {
                "★★★★★ only"
            }
        );

        // Space accounting: safe vs review vs total
        let owned_cleanable: Vec<rules::ClassifiedFile> =
            cleanable.iter().map(|f| (*f).clone()).collect();
        let cs = confidence::ConfidenceSummary::from_files(&owned_cleanable);

        let safe_size: u64 = owned_cleanable
            .iter()
            .filter(|f| matches!(confidence::confidence(f), confidence::Tier::Maximum))
            .map(|f| f.size)
            .sum();
        let review_size: u64 = owned_cleanable
            .iter()
            .filter(|f| {
                matches!(
                    confidence::confidence(f),
                    confidence::Tier::High | confidence::Tier::Moderate
                )
            })
            .map(|f| f.size)
            .sum();

        println!(
            "  Safe (★★★★★):  {} files ({})",
            cs.maximum,
            simulator::human_size(safe_size)
        );
        println!(
            "  Review (★★★+): {} files ({})",
            cs.high + cs.moderate,
            simulator::human_size(review_size)
        );
        println!(
            "  Total:         {} files ({})",
            cleanable.len(),
            simulator::human_size(to_clean_size)
        );
        println!(
            "  Would skip:    {} files ({})",
            skipped.len(),
            simulator::human_size(skipped_size)
        );

        // Domain breakdown (summary-first)
        let domains = domain::summarize(&owned_cleanable);
        if !domains.is_empty() {
            println!("\n  WOULD CLEAN BY DOMAIN\n");
            for d in domains.iter().take(10) {
                // v6.4.0: Use dominant decision + risk_score to compute tier.
                // Pure risk_score mapping ignores the bridge override that
                // changes toolchain files from Safe to LowRisk.
                // LOWRISK/MIXED-dominant domains (e.g. toolchains) → ★★★★ High.
                let tier = if d.dominant_decision == "SAFE" {
                    confidence::Tier::Maximum
                } else if d.dominant_decision == "BLOCKED" {
                    confidence::Tier::Protected
                } else if d.dominant_decision == "LOWRISK" || d.dominant_decision == "MIXED" {
                    confidence::Tier::High // ★★★★ — requires --smart
                } else if d.risk_score < 0.15 {
                    confidence::Tier::Maximum
                } else if d.risk_score < 0.35 {
                    confidence::Tier::High
                } else {
                    confidence::Tier::Moderate
                };
                println!(
                    "  {} {:<35} {:>5} files  {}",
                    tier.stars(),
                    d.domain,
                    d.file_count as i64,
                    simulator::human_size(d.total_size)
                );
            }
            if domains.len() > 10 {
                println!("  ... and {} more domains", domains.len() - 10);
            }
        }

        // Top contributors (v6.2.1)
        let top = pipeline::top_contributors(&owned_cleanable, 8);
        if !top.is_empty() {
            println!("\n  TOP CONTRIBUTORS\n");
            for (name, count, size) in &top {
                println!(
                    "  {:<40} {:>4} files  {}",
                    name,
                    count,
                    simulator::human_size(*size)
                );
            }
        }

        // File list only with --verbose
        if verbose && !cleanable.is_empty() {
            println!("\n  FILES\n");
            for f in cleanable.iter().take(50) {
                let tier = confidence::confidence(f);
                println!(
                    "  {} {}  {}",
                    tier.stars(),
                    simulator::human_size(f.size),
                    f.path
                );
            }
            if cleanable.len() > 50 {
                println!("  ... and {} more files", cleanable.len() - 50);
            }
        } else if !cleanable.is_empty() && !verbose {
            println!("\n  (use --verbose for file list)");
        }
        return;
    }

    // Actual clean
    let mut snap = snapshot::Snapshot::new();
    for f in &classified {
        snap.add(&f.path, f.size, None);
    }
    let _snap_path = snap.save().unwrap_or_default();
    let snap_id = pipeline::chrono_now();
    let report = cleaner::clean(&classified, smart, force);

    if json {
        let out = serde_json::json!({
            "snapshot_id": snap_id,
            "files_removed": report.files_removed,
            "bytes_freed": report.bytes_freed,
            "files_skipped": report.files_skipped,
            "bytes_skipped": report.bytes_skipped,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }

    println!();
    println!("\nCLEAN COMPLETE");
    println!("──────────────");
    println!(
        "  Removed:  {} files ({})",
        report.files_removed,
        simulator::human_size(report.bytes_freed)
    );
    println!(
        "  Skipped:  {} files ({})",
        report.files_skipped,
        simulator::human_size(report.bytes_skipped)
    );
    if !report.errors.is_empty() {
        println!(
            "║  Errors:   {:>5}                                   ║",
            report.errors.len()
        );
    }
    println!("  Snapshot:  {}", snap.id);
    println!("  Undo:      zacxiom undo {}", snap.id);

    if !report.errors.is_empty() {
        println!("\n  Errors:");
        for e in &report.errors {
            println!("    {} → {}", e.path, e.error);
        }
    }
}
