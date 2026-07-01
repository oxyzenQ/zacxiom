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
) {
    let mut prog = progress::Progress::new(json);
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
    let classified = pipeline::classify(entries, &ctx, threads, cfg);
    prog.advance();
    prog.advance();
    prog.done();

    // v13.1: Update cache with current scan results (best-effort, non-blocking)
    if use_cache {
        let cache_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for entry in scanner::scan(&roots, depth, effective_min_size, true, &exclude) {
            let path_str = entry.path.to_string_lossy().into_owned();
            let mtime = scan_cache::get_mtime_secs(&entry.path).unwrap_or(0);
            let inode = scan_cache::get_inode(&entry.path);
            cache.insert(&path_str, entry.size, mtime, inode);
        }
        cache.last_updated = cache_updated;
        // Prune missing entries periodically
        if cache.files.len() > 10_000 {
            cache.prune_missing();
        }
        cache.save();
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
}
