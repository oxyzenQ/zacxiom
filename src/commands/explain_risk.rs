// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom explain-risk` — v10: Detailed risk reasoning.

use crate::color;
use crate::evidence;
use std::path::PathBuf;

pub fn run_explain_risk(path: String) {
    let target = PathBuf::from(&path);
    if !target.exists() {
        eprintln!("No such path: {path}");
        std::process::exit(1);
    }

    let plan = crate::planner::plan(&target);
    let is_rust = target.join("Cargo.toml").exists();

    let report = if is_rust {
        evidence::collect_rust_evidence(&target, plan.estimated_reclaimable_bytes)
    } else {
        evidence::collect_generic_evidence(&target, plan.safe_to_clean, plan.safe_to_clean)
    };

    println!(
        "{}",
        color::section_header(&format!("RISK ANALYSIS: {}", path))
    );
    println!();

    // Risk breakdown
    println!("{}", report.risk.render());
    println!();

    // Cross-reference with planner
    println!("Planner Cross-Reference");
    println!("{}", "─".repeat(40));
    println!(
        "  Safety verdict:     {}",
        if plan.safe_to_clean { "Safe" } else { "Unsafe" }
    );
    println!("  Risk level:         {}", plan.risk_level.display());
    if !plan.reason.is_empty() {
        println!("  Reason:             {}", plan.reason);
    }
    if !plan.recommendation.is_empty() {
        println!("  Recommendation:     {}", plan.recommendation);
    }
    println!();

    // Confidence summary
    println!("Confidence Summary");
    println!("{}", "─".repeat(40));
    println!("  Confidence:         {}%", report.confidence.final_score);
    println!("  Evidence items:     {}", report.evidence.items.len());
    println!();

    println!("Every risk factor is auditable. No hidden heuristics.");
}
