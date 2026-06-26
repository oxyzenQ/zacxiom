// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom explain-confidence` — v10: Evidence-based confidence breakdown.

use crate::color;
use crate::evidence::{self, EvidenceCategory};
use std::path::PathBuf;

pub fn run_explain_confidence(path: String) {
    let target = PathBuf::from(&path);
    if !target.exists() {
        eprintln!("No such path: {path}");
        std::process::exit(1);
    }

    // Detect project type for evidence collection
    let is_rust = target.join("Cargo.toml").exists();
    let is_node = target.join("package.json").exists();

    let report = if is_rust {
        let size = crate::planner::plan(&target).estimated_reclaimable_bytes;
        evidence::collect_rust_evidence(&target, size)
    } else if is_node {
        evidence::collect_generic_evidence(&target, true, true)
    } else {
        evidence::collect_generic_evidence(&target, false, true)
    };

    println!(
        "{}",
        color::section_header(&format!("CONFIDENCE: {}", path))
    );
    println!();

    // Evidence section
    let ecosystem_evidence = report.evidence.by_category(EvidenceCategory::Ecosystem);
    let regen_evidence = report.evidence.by_category(EvidenceCategory::Regeneration);
    let safety_evidence = report.evidence.by_category(EvidenceCategory::Safety);
    let ownership_evidence = report.evidence.by_category(EvidenceCategory::Ownership);

    if !ecosystem_evidence.is_empty() {
        println!("Ecosystem Evidence");
        println!("{}", "─".repeat(40));
        for e in &ecosystem_evidence {
            println!("  {} {}", e.icon(), e.title);
        }
        println!();
    }

    if !ownership_evidence.is_empty() {
        println!("Ownership Evidence");
        println!("{}", "─".repeat(40));
        for e in &ownership_evidence {
            println!("  {} {}", e.icon(), e.title);
        }
        println!();
    }

    if !regen_evidence.is_empty() {
        println!("Regeneration Evidence");
        println!("{}", "─".repeat(40));
        for e in &regen_evidence {
            println!("  {} {}", e.icon(), e.title);
        }
        println!();
    }

    if !safety_evidence.is_empty() {
        println!("Safety Evidence");
        println!("{}", "─".repeat(40));
        for e in &safety_evidence {
            println!("  {} {}", e.icon(), e.title);
        }
        println!();
    }

    // Confidence breakdown
    println!("{}", report.confidence.render());
    println!();

    println!("No hidden scoring.");
}

/// Explain confidence for a path (shared with explain-risk output).
pub fn explain_confidence(path: &std::path::Path) -> Option<evidence::EvidenceReport> {
    if !path.exists() {
        return None;
    }

    let is_rust = path.join("Cargo.toml").exists();
    if is_rust {
        let size = crate::planner::plan(path).estimated_reclaimable_bytes;
        Some(evidence::collect_rust_evidence(path, size))
    } else {
        Some(evidence::collect_generic_evidence(
            path,
            crate::planner::plan(path).safe_to_clean,
            true,
        ))
    }
}
