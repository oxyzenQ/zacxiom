// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom scan` — filesystem scan command.

use crate::confidence;
use crate::config::Config;
use crate::display;
use crate::domain;
use crate::inspect;
use crate::pipeline::{self, get_open_files, RunContext};
use crate::progress;
use crate::scan_cache;
use crate::scanner;
use crate::summary;

#[allow(clippy::too_many_arguments)]
pub fn run_scan(
    paths: Vec<String>,
    depth: usize,
    min_size: u64,
    json: bool,
    verbose: bool,
    profile: &str,
    cfg: &Config,
    cli_exclude: &[String],
    use_cache: bool,
    suggest: bool,
) {
    let mut prog = progress::Progress::new(json || std::env::var("ZACXIOM_QUIET").is_ok());
    let ctx = RunContext::new(profile);
    let roots = pipeline::resolve_roots(paths);

    // v13: Warn when scanning user-content directories
    if cfg.scan.warn_user_dirs && !json {
        for root in &roots {
            if scanner::is_user_content_dir(root) {
                eprintln!(
                    "⚠ Warning: scanning user-content directory: {}",
                    root.display()
                );
                eprintln!(
                    "  Use --exclude to skip subdirectories or patterns (e.g. --exclude \"*.iso\")"
                );
                eprintln!(
                    "  Or add to config: [scan].exclude = [\"{}\"]",
                    root.display()
                );
                break;
            }
        }
    }

    // v13: Build exclude filter from config + CLI
    let exclude = pipeline::build_exclude_filter(cfg, cli_exclude);
    let effective_min_size = if min_size > 1 {
        min_size
    } else {
        cfg.scan.min_size
    };

    // v13.1: Load incremental scan cache (unless --no-cache)
    let mut cache = if use_cache {
        scan_cache::ScanCache::load()
    } else {
        scan_cache::ScanCache::new()
    };

    let entries = scanner::scan(&roots, depth, effective_min_size, true, &exclude);
    prog.advance();
    let threads = pipeline::optimal_threads_with_config(entries.len(), cfg.scan.max_threads);
    prog.set_threads(threads);
    let classified = pipeline::classify(entries, &ctx, threads, cfg, &cache);
    prog.advance();
    prog.advance();
    prog.done();

    // v14.1: Update cache with full classification results for cache-aware future scans.
    // Stores (decision, risk_score, engine_category, engine_confidence, domain) per file.
    // Next scan with unchanged files → 100% cache hits → skip entire classification pipeline.
    if use_cache {
        let cache_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for f in &classified {
            let mtime = scan_cache::get_mtime_secs(std::path::Path::new(&f.path)).unwrap_or(0);
            let inode = scan_cache::get_inode(std::path::Path::new(&f.path));
            cache.insert_classified(
                &f.path,
                f.size,
                mtime,
                inode,
                &format!("{:?}", f.decision),
                f.risk_score,
                &f.engine_category,
                f.engine_confidence,
                &f.cache_domain.to_string(),
            );
        }
        cache.last_updated = cache_updated;
        // Prune missing entries periodically
        if cache.files.len() > 10_000 {
            let pruned = cache.prune_missing();
            if pruned > 0 && !json {
                eprintln!("  Cache pruned {pruned} stale entries");
            }
        }
        cache.save();
    }

    // v14.1: Show cache stats if enabled and not JSON
    if use_cache && !json {
        let (hits, misses, rate) = scan_cache::get_stats();
        if hits + misses > 0 {
            eprintln!("  Cache: {hits} hits, {misses} misses ({rate:.0}% hit rate)");
        }
    }

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

    // v13.2: Smart suggestions — show stale directory hints (opt-in via --suggest)
    if suggest && !json {
        print_smart_suggestions(&roots, &classified);
    }
}

/// v13.2: Print smart suggestions for stale directories and cleanup hints.
/// Only shows actionable insights — never nags.
fn print_smart_suggestions(
    _roots: &[std::path::PathBuf],
    classified: &[crate::rules::ClassifiedFile],
) {
    use std::collections::HashMap;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let thirty_days_secs: u64 = 30 * 86400;

    // Group files by top-level directory under each root
    let mut dir_stats: HashMap<String, (usize, u64, u64)> = HashMap::new(); // dir → (count, total_size, oldest_mtime)
    for f in classified {
        if let Some(parent) = std::path::Path::new(&f.path).parent() {
            let dir_str = parent.to_string_lossy().to_string();
            let entry = dir_stats.entry(dir_str).or_insert((0, 0, u64::MAX));
            entry.0 += 1;
            entry.1 += f.size;
            // Track oldest mtime
            if let Ok(meta) = std::fs::metadata(&f.path) {
                if let Ok(mtime) = meta.modified() {
                    if let Ok(secs) = mtime.duration_since(std::time::UNIX_EPOCH) {
                        if secs.as_secs() < entry.2 {
                            entry.2 = secs.as_secs();
                        }
                    }
                }
            }
        }
    }

    // Find stale directories (oldest file > 30 days, > 10 files, > 1MB)
    let mut stale: Vec<_> = dir_stats
        .iter()
        .filter(|(_, (count, size, oldest))| {
            *count > 10 && *size > 1_000_000 && now.saturating_sub(*oldest) > thirty_days_secs
        })
        .collect();
    stale.sort_by_key(|(_, (_, size, _))| std::cmp::Reverse(*size));

    if !stale.is_empty() {
        println!("\n  ━━━ SMART SUGGESTIONS ━━━");
        println!("  Directories with stale files (untouched >30 days, >1MB):");
        for (dir, (count, size, _)) in stale.iter().take(5) {
            let size_str = crate::simulator::human_size(*size);
            println!("    {size_str:>10}  {count:>5} files  {dir}");
        }
        if stale.len() > 5 {
            println!("    ... and {} more", stale.len() - 5);
        }
        println!();
        println!("  Review with: zacxiom explain <path>");
        println!("  Clean with:  zacxiom clean --smart --yes");
    }
}
