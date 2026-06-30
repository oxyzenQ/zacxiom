// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom clean` — cleanup execution command.
//!
//! v13: Safety-first redesign:
//! - Default dry-run on first use (unless --yes)
//! - Confirmation prompt for --smart and --force (unless --yes)
//! - --force NO LONGER allows HighRisk files (those need manual `rm`)
//! - safety::validate_clean is called before any deletion
//! - Exclude filter respected (config + CLI + .zacxiomignore)
//! - Protected extensions (.iso, .vmdk, .pem, etc.) NEVER cleaned

use crate::cleaner::{self, CleanOptions};
use crate::confidence;
use crate::config::Config;
use crate::domain;
use crate::pipeline::{self, RunContext};
use crate::progress;
use crate::rules;
use crate::safety;
use crate::scanner;
use crate::simulator;
use crate::snapshot;

use std::io::{self, Write};

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
    cfg: &Config,
    cli_exclude: &[String],
    yes: bool,
    fail_fast: bool,
    include_patterns: &[String],
) {
    let mut prog = progress::Progress::new(json);
    let ctx = RunContext::new(profile);
    let roots = pipeline::resolve_roots(paths);
    let exclude = pipeline::build_exclude_filter(cfg, cli_exclude);
    let effective_min_size = cfg.scan.min_size;
    let entries = scanner::scan(&roots, depth, effective_min_size, true, &exclude);
    prog.advance();
    let threads = pipeline::optimal_threads(entries.len());
    prog.set_threads(threads);
    let mut classified = pipeline::classify(entries, &ctx, threads, cfg);
    prog.advance();
    prog.advance();
    prog.done();

    // v13: --include whitelist mode — only keep files matching the patterns
    if !include_patterns.is_empty() {
        classified.retain(|f| {
            let path = std::path::Path::new(&f.path);
            let path_str = path.to_string_lossy();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            include_patterns.iter().any(|pat| {
                if let Ok(glob) = globset::Glob::new(pat) {
                    let m = glob.compile_matcher();
                    m.is_match(path_str.as_ref()) || m.is_match(name)
                } else {
                    false
                }
            })
        });
    }

    // v13: Determine if this is a first-run (no snapshots exist yet)
    let first_run = snapshot::Snapshot::list_all().is_empty();
    // v13: Apply config default_mode if no explicit flag given
    let effective_smart = smart || (!force && cfg.clean.default_mode == "smart");
    let effective_force = force;

    // v13: Default dry-run on first use (Option A — safe-default)
    // Unless --yes is given, first-time users get a dry-run preview
    let effective_dry_run = dry_run || (cfg.clean.first_run_dry_run && first_run && !yes && !json);

    if effective_dry_run && !dry_run && first_run && !yes && !json {
        println!();
        println!("━━━ FIRST RUN — DRY RUN PREVIEW ━━━");
        println!("  This is your first clean operation. Zacxiom is showing a preview");
        println!("  instead of deleting files. To actually clean, re-run with --yes:");
        println!("    zacxiom clean --yes");
        println!();
    }

    if effective_dry_run {
        run_dry_run(&classified, effective_smart, effective_force, json, verbose);
        return;
    }

    // v13: Pre-deletion safety gate — wire safety::validate_clean (was dead code!)
    let safety_check = safety::validate_clean(&classified, effective_smart, effective_force, true);
    if !safety_check.passed {
        if json {
            let out = serde_json::json!({
                "status": "aborted",
                "reason": "safety_check_failed",
                "violations": safety_check.violations,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        } else {
            eprintln!();
            eprintln!("━━━ SAFETY CHECK FAILED — ABORTING ━━━");
            for v in &safety_check.violations {
                eprintln!("  • {v}");
            }
            eprintln!();
            eprintln!("  No files were modified. Fix the violations and try again.");
        }
        std::process::exit(3);
    }

    // v13: Confirmation prompt for --smart and --force (unless --yes)
    let needs_confirmation =
        (effective_force || effective_smart) && cfg.clean.require_confirmation && !yes;
    if needs_confirmation
        && !json
        && !confirm_deletion(&classified, effective_smart, effective_force)
    {
        eprintln!();
        eprintln!("  Aborted. No files were modified.");
        std::process::exit(0);
    }

    // Actual clean — v10: trash-based recovery
    let trash_base = snapshot::trash_dir();
    let mut snap = snapshot::Snapshot::new();
    let trash_for_snap = trash_base.join(&snap.id);
    let snap_dir = snapshot::snapshot_dir();
    let _ = std::fs::create_dir_all(&snap_dir);
    let snap_path = snap_dir.join(&snap.id);
    let report = cleaner::clean(
        &classified,
        &CleanOptions {
            smart: effective_smart,
            force: effective_force,
            fail_fast,
            verify_checksum: cfg.trash.verify_checksum,
            show_progress: !json,
        },
        &trash_for_snap,
        &mut snap,
        &snap_path,
    );

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

    // v12: Snapshot already populated incrementally by cleaner::clean().
    // Save the final authoritative version (incremental saves were best-effort).
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
    println!("  Stored in: ~/.cache/zacxiom/snapshots/");

    // Show top-removed categories for user confidence
    let removed_paths: std::collections::HashSet<&str> = report
        .trash_entries
        .iter()
        .map(|e| e.original_path.as_str())
        .collect();
    let mut domain_counts: std::collections::HashMap<String, (usize, u64)> =
        std::collections::HashMap::new();
    for f in &classified {
        if removed_paths.contains(f.path.as_str()) {
            let name = crate::rules::CacheDomain::display_name(&f.cache_domain);
            let entry = domain_counts.entry(name.to_string()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += f.size;
        }
    }
    if !domain_counts.is_empty() {
        let mut sorted: Vec<_> = domain_counts.into_iter().collect();
        sorted.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
        println!();
        println!("  Top removed:");
        for (domain, (count, size)) in sorted.iter().take(5) {
            println!(
                "    {:<24} {:>6} files  {}",
                domain,
                count,
                simulator::human_size(*size)
            );
        }
    }

    report_errors(&report);
}

/// v13: Interactive confirmation prompt for --smart and --force modes.
/// Returns true if user confirmed, false if they declined.
fn confirm_deletion(classified: &[rules::ClassifiedFile], smart: bool, force: bool) -> bool {
    let cleanable: Vec<_> = classified
        .iter()
        .filter(|f| f.decision.is_cleanable(smart, force))
        .collect();

    let total_size: u64 = cleanable.iter().map(|f| f.size).sum();
    let mode = if force {
        "FORCE"
    } else if smart {
        "SMART"
    } else {
        "SAFE"
    };

    println!();
    println!("━━━ CONFIRMATION REQUIRED ({mode} mode) ━━━");
    println!("  Files to delete : {}", cleanable.len());
    println!("  Total size      : {}", simulator::human_size(total_size));
    println!();
    println!("  Top 5 paths:");
    let mut sorted = cleanable.clone();
    sorted.sort_by_key(|f| std::cmp::Reverse(f.size));
    for f in sorted.iter().take(5) {
        println!("    {}  {}", simulator::human_size(f.size), f.path);
    }
    if cleanable.len() > 5 {
        println!("    ... and {} more", cleanable.len() - 5);
    }
    println!();
    println!("  All deleted files are recoverable via `zacxiom undo` (trash-based).");
    println!();

    if force {
        // --force requires typing "DELETE" (stronger confirmation)
        print!("  Type \"DELETE\" to confirm (or anything else to abort): ");
    } else {
        // --smart requires typing "yes"
        print!("  Type \"yes\" to confirm (or anything else to abort): ");
    }
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    let input = input.trim();
    if force {
        input == "DELETE"
    } else {
        input == "yes" || input == "y" || input == "YES"
    }
}

/// Run dry-run preview (extracted from original clean.rs).
fn run_dry_run(
    classified: &[rules::ClassifiedFile],
    smart: bool,
    force: bool,
    json: bool,
    verbose: bool,
) {
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

    // Time estimate
    let estimate_secs = cleanable.len() as f64 / 500.0;
    let time_str = if estimate_secs < 1.0 || cleanable.len() < 10 {
        "< 1 second".to_string()
    } else if estimate_secs < 60.0 {
        format!("~{:.0} seconds", estimate_secs.ceil())
    } else {
        format!("~{:.0} minutes", (estimate_secs / 60.0).ceil())
    };

    println!();
    println!("DRY RUN — Estimated Cleanup");
    println!("───────────────────────────");
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
    println!("  ≈ {} files", cleanable.len());
    println!("  ≈ {} recovered", simulator::human_size(to_clean_size));
    println!("  ≈ {}", time_str);

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
            let tier = if d.dominant_decision == "SAFE" {
                confidence::Tier::Maximum
            } else if d.dominant_decision == "BLOCKED" {
                confidence::Tier::Protected
            } else if d.dominant_decision == "LOWRISK" || d.dominant_decision == "MIXED" {
                confidence::Tier::High
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
