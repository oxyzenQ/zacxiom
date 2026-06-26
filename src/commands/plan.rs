// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom plan` — cleanup plan command.
//!
//! v8.6: Workspace-aware planning — detects multi-project directories
//! and provides cross-project cleanup recommendations.

use crate::advisor;
use crate::planner;
use crate::workspace;
use std::path::{Path, PathBuf};

pub fn run_plan(path: String) {
    // v10: resolve relative paths to absolute
    let target = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&path)
    };
    if !target.exists() {
        eprintln!("No such path: {path}");
        if Path::new(&path).is_relative() {
            eprintln!("  (relative paths are resolved from the current directory)");
        }
        std::process::exit(1);
    }
    // v8.3.1: P1 — dangerous path hard block
    if let Err(blocked) = planner::check_path_blocked(&target) {
        println!("{}", planner::render_blocked(&blocked));
        std::process::exit(1);
    }

    if target.is_dir() {
        // v8.6: Check if this is a workspace (contains multiple projects)
        let ws = workspace::discover_workspace(&target);

        if ws.project_count > 1 {
            // Multi-project workspace — show workspace summary + cross-project recs
            println!("{}", workspace::render_workspace_summary(&ws));

            // Cross-project recommendations
            let recs = workspace::cross_project_recommendations(&ws);
            if !recs.is_empty() {
                println!(
                    "\n{}",
                    crate::color::section_header("CROSS-PROJECT RECOMMENDATIONS")
                );
                for (rec, projects) in &recs {
                    println!("  {}", rec);
                    for p in projects {
                        println!("    - {}", p);
                    }
                }
            }

            // Also show advisor for the root (aggregated view)
            let adv = advisor::advise(&target);
            if !adv.opportunities.is_empty() {
                println!(
                    "\n{}",
                    crate::color::section_header("AGGREGATED RECOMMENDATIONS")
                );
                println!("{}", advisor::render_advisor(&adv, &target));
            }
        } else {
            // Single project or empty directory — use existing advisor/planner
            let adv = advisor::advise(&target);
            if !adv.opportunities.is_empty() {
                println!("{}", advisor::render_advisor(&adv, &target));
            } else {
                // No opportunities found — fall back to single-path planner
                let cleanup_plan = planner::plan(&target);
                println!("{}", planner::render_plan(&cleanup_plan, &target));
            }
        }
    } else {
        let cleanup_plan = planner::plan(&target);
        println!("{}", planner::render_plan(&cleanup_plan, &target));
    }
}
