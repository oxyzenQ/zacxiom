// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — Ownership detection, risk scoring, and size computation.

use std::fs;
use std::path::Path;

use crate::discovery;
use crate::engine::{types::RiskLevel, Category, ClassificationResult};
use crate::impact;

/// Boost confidence when project ownership is discovered.
pub(crate) fn boost_confidence_from_discovery(eng: &mut ClassificationResult) {
    if let Some(project) = discovery::find_project_for_path(&eng.path) {
        if eng.confidence_score < 95 {
            eng.confidence_score = (eng.confidence_score + 10).min(99);
        }
        let reason = format!(
            "Project ownership discovered: {} ({})",
            project.name,
            project.ecosystem.display()
        );
        if !eng.confidence_reasons.contains(&reason) {
            eng.confidence_reasons.push(reason);
        }
    }
}

/// Compute risk level from engine classification and impact analysis.
pub(crate) fn compute_risk(
    eng: &ClassificationResult,
    impact_analysis: &impact::ImpactAnalysis,
) -> RiskLevel {
    // Map engine risk level directly for protected/critical categories
    if eng.category.is_protected() || matches!(eng.category, Category::UserHomeRoot) {
        return RiskLevel::Critical;
    }

    // Configuration categories store user-curated settings — deleting loses
    // customization.  They are never safe to clean, so the minimum risk
    // must be High (not Moderate) to stay consistent with the safety verdict.
    if matches!(
        eng.category,
        Category::ApplicationConfiguration
            | Category::ShellConfiguration
            | Category::EnvironmentFile
    ) {
        return RiskLevel::High;
    }

    // Map from impact level to our risk level
    match impact_analysis.level {
        impact::ImpactLevel::Critical => RiskLevel::Critical,
        impact::ImpactLevel::High => RiskLevel::High,
        impact::ImpactLevel::Medium => RiskLevel::Moderate,
        impact::ImpactLevel::Low => {
            if eng.category.is_cleanable() {
                RiskLevel::Low
            } else {
                RiskLevel::Moderate
            }
        }
    }
}

/// Compute the estimated size of a path in bytes.
pub(crate) fn compute_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }

    // Directory — walk and sum file sizes (read-only, no mutation)
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entrypath = entry.path();
            if entrypath.is_file() {
                total += fs::metadata(&entrypath).map(|m| m.len()).unwrap_or(0);
            } else if entrypath.is_dir() {
                total += compute_size(&entrypath);
            }
        }
    }
    total
}
