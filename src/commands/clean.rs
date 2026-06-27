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
        // v10: JSON output support for dry-run
        if json {
            let cleanable: Vec<_> = classified
                .iter()
                .filter(|f| f.decision.is_cleanable(smart, force))
                .collect();
            let skipped: Vec<_> = classified
                .iter()
                .filter(|f| !f.decision.is_cleanable(smart, force))
                .collect();
            let mode = if force {
                "force"
            } else if smart {
                "smart"
            } else {
                "safe"
            };
            let out = serde_json::json!({
                "mode": mode,
                "total_cleanable": cleanable.len(),
                "total_cleanable_size": cleanable.iter().map(|f| f.size).sum::<u64>(),
                "total_skipped": skipped.len(),
                "total_skipped_size": skipped.iter().map(|f| f.size).sum::<u64>(),
                "files": cleanable.iter().map(|f| serde_json::json!({
                    "path": f.path,
                    "size": f.size,
                    "decision": format!("{:?}", f.decision),
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            return;
        }
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

    // Actual clean — v10: trash-based recovery
    let trash_base = snapshot::trash_dir();
    let mut snap = snapshot::Snapshot::new();
    let trash_for_snap = trash_base.join(&snap.id);
    let report = cleaner::clean(&classified, smart, force, &trash_for_snap);

    // v10.0.0-rc1: Don't create snapshot if nothing was removed
    if report.files_removed == 0 {
        // Clean up empty trash directory
        let _ = std::fs::remove_dir_all(&trash_for_snap);

        if json {
            let out = serde_json::json!({
                "snapshot_id": null,
                "files_removed": 0,
                "bytes_freed": 0,
                "files_skipped": report.files_skipped,
                "bytes_skipped": report.bytes_skipped,
                "message": "No files were removed. No snapshot created.",
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            return;
        }

        println!();
        println!("\nCLEAN COMPLETE");
        println!("──────────────");
        println!("  Removed:  0 files (0.00 B)");
        println!(
            "  Skipped:  {} files ({})",
            report.files_skipped,
            simulator::human_size(report.bytes_skipped)
        );
        println!("  No snapshot created (nothing to undo).");
        if !report.errors.is_empty() {
            report_errors(&report);
        }
        return;
    }

    // Record trash paths in snapshot
    for (orig, trash) in &report.trash_paths {
        snap.add(orig, 0, Some(trash.clone()));
    }
    // Record skipped files for auditing
    for f in &classified {
        if !f.decision.is_cleanable(smart, force) {
            snap.add_skipped(&f.path, f.size);
        }
    }
    let _snap_path = snap.save().unwrap_or_default();

    if json {
        let out = serde_json::json!({
            "snapshot_id": snap.id,
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
    println!("  Undo:      zacxiom undo --id {}", snap.id);

    report_errors(&report);
}

/// Display categorized error summary + first 5 details.
fn report_errors(report: &cleaner::CleanReport) {
    if report.errors.is_empty() {
        return;
    }
    if !report.error_counts.is_empty() {
        let mut sorted: Vec<_> = report.error_counts.iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        println!();
        println!("  Error summary:");
        for (cat, count) in &sorted {
            println!("    {cat}: {count}");
        }
    }
    println!("\n  Details:");
    for e in report.errors.iter().take(5) {
        println!("    {} → {}", e.path, e.error);
    }
    if report.errors.len() > 5 {
        println!("    ... and {} more", report.errors.len() - 5);
    }
}
