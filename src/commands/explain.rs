// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom explain` — detailed path classification command.

use crate::discovery;
use crate::explain;
use crate::pipeline::{self, RunContext};
use crate::scanner;
use std::path::{Path, PathBuf};

/// v10: Resolve a path string to an absolute path, handling relative paths.
fn resolve_path(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub fn run_explain(path: &str) {
    // v10: resolve relative paths to absolute for consistent behavior
    let target = resolve_path(path);
    if !target.exists() {
        eprintln!("No such path: {path}");
        if Path::new(path).is_relative() {
            eprintln!("  (relative paths are resolved from the current directory)");
        }
        std::process::exit(1);
    }

    let ctx = RunContext::new("dev");

    if target.is_file() {
        // Single file — create a scan entry directly, never scan parent dir
        let size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
        let entry = scanner::ScanEntry {
            path: target.clone(),
            size,
        };
        let entries = vec![entry];
        let threads = 1;
        let classified = pipeline::classify(entries, &ctx, threads);
        let mut eng = crate::engine::classify(&target);
        explain::upgrade_workspace(&mut eng);
        explain::fix_home_inheritance(&mut eng);
        boost_confidence_from_discovery(&mut eng);
        println!(
            "{}",
            explain::render_card(
                &explain::explain_path(path, &classified, Some(&eng)),
                Some(&eng)
            )
        );
        return;
    }

    // Directory — scan only that directory, not parent; use sufficient depth
    let roots = vec![target];
    let entries = scanner::scan(&roots, 8, 1, true);
    let threads = pipeline::optimal_threads(entries.len());
    let classified = pipeline::classify(entries, &ctx, threads);

    let mut eng = crate::engine::classify(&PathBuf::from(path));
    explain::upgrade_workspace(&mut eng);
    explain::fix_home_inheritance(&mut eng);
    boost_confidence_from_discovery(&mut eng);
    println!(
        "{}",
        explain::render_card(
            &explain::explain_path(path, &classified, Some(&eng)),
            Some(&eng)
        )
    );
}

/// v8.0: Boost confidence when project ownership is discovered.
pub fn boost_confidence_from_discovery(eng: &mut crate::engine::ClassificationResult) {
    if let Some(project) = discovery::find_project_for_path(&eng.path) {
        // Only boost if not already at max
        if eng.confidence_score < 95 {
            eng.confidence_score = (eng.confidence_score + 10).min(99);
        }
        let reason = format!(
            "✓ Project ownership discovered: {} ({})",
            project.name,
            project.ecosystem.display()
        );
        if !eng.confidence_reasons.contains(&reason) {
            eng.confidence_reasons.push(reason);
        }
    }
}
