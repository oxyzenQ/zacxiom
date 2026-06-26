// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom plan` — cleanup plan command.

use crate::advisor;
use crate::planner;
use std::path::PathBuf;

pub fn run_plan(path: String) {
    let target = PathBuf::from(&path);
    if !target.exists() {
        eprintln!("No such path: {path}");
        std::process::exit(1);
    }
    // v8.3.1: P1 — dangerous path hard block
    if let Err(blocked) = planner::check_path_blocked(&target) {
        println!("{}", planner::render_blocked(&blocked));
        std::process::exit(1);
    }
    // v8.4: Cleanup Advisor for directories
    if target.is_dir() {
        let adv = advisor::advise(&target);
        if !adv.opportunities.is_empty() {
            println!("{}", advisor::render_advisor(&adv, &target));
        } else {
            // No opportunities found — fall back to single-path planner
            let cleanup_plan = planner::plan(&target);
            println!("{}", planner::render_plan(&cleanup_plan, &target));
        }
    } else {
        let cleanup_plan = planner::plan(&target);
        println!("{}", planner::render_plan(&cleanup_plan, &target));
    }
}
