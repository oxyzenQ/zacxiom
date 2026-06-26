// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — Safety determination and core planning logic.

use std::path::Path;

use crate::color;
use crate::engine::{self, Category, ClassificationResult};
use crate::impact;
use crate::ownership;

use super::notes::{build_expected_result, build_notes, find_safer_children};
use super::ownership as planner_ownership;
use super::regeneration::build_ecosystem_recommendation;
use super::types::{is_dangerous_system_path, BlockedPath, CleanupPlan};

/// Check if a path is blocked and return the block info.
pub fn check_path_blocked(path: &Path) -> Result<(), BlockedPath> {
    if let Some(blocked) = is_dangerous_system_path(path) {
        return Err(BlockedPath {
            path: blocked.to_string(),
            reason: "System-critical path.".into(),
            suggestions: vec![
                "zacxiom scan",
                "zacxiom plan ~/.cache",
                "zacxiom plan ~/Downloads",
            ],
        });
    }
    Ok(())
}

/// Render a blocked path error message.
pub fn render_blocked(blocked: &BlockedPath) -> String {
    let mut out = String::new();
    out.push_str(&color::section_header("ERROR"));
    out.push_str(&format!(
        "  Refusing to create cleanup plan for:\n\n    {}\n\n",
        blocked.path
    ));
    out.push_str(&format!("  Reason:\n    {}\n\n", blocked.reason));
    out.push_str(
        "  Why:\n    Cleanup recommendations on this path may be misleading or dangerous.\n\n",
    );
    out.push_str("  Try:\n");
    for suggestion in &blocked.suggestions {
        out.push_str(&format!("    {}\n", suggestion));
    }
    out
}

/// Generate a cleanup plan for the given path.
///
/// Consumes outputs from classification, ownership, and impact
/// without duplicating their logic.
pub fn plan(path: &Path) -> CleanupPlan {
    let mut eng = engine::classify(path);
    planner_ownership::boost_confidence_from_discovery(&mut eng);

    let ownership = ownership::detect_project_ownership(path);
    let impact_analysis = impact::analyze_impact(path, &eng);

    let safe_to_clean = determine_safety(path, &eng, &ownership);
    let risk_level = planner_ownership::compute_risk(&eng, &impact_analysis);
    let estimated_reclaimable_bytes = planner_ownership::compute_size(path);
    let (recommendation, reason, regeneration, suggested_commands) =
        build_ecosystem_recommendation(path, &eng, &ownership);
    let notes = build_notes(&eng, &ownership, &impact_analysis);
    let safer_alternatives = if safe_to_clean {
        Vec::new()
    } else {
        find_safer_children(path, &eng, &ownership)
    };
    let expected_result = build_expected_result(
        path,
        &eng,
        &ownership,
        safe_to_clean,
        &estimated_reclaimable_bytes,
    );

    CleanupPlan {
        safe_to_clean,
        risk_level,
        estimated_reclaimable_bytes,
        recommendation,
        reason,
        regeneration,
        suggested_commands,
        notes,
        safer_alternatives,
        expected_result,
    }
}

/// Determine if a path is safe to clean based on its category and ownership.
fn determine_safety(
    target: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
) -> bool {
    // Unsafe categories — never clean
    match eng.category {
        Category::SystemBinary
        | Category::SystemConfiguration
        | Category::SystemData
        | Category::VirtualFilesystem
        | Category::SecurityCredential
        | Category::UserHomeRoot
        | Category::UserDocument
        | Category::UserMedia
        | Category::UserDesktop
        | Category::ShellConfiguration
        | Category::ApplicationConfiguration
        | Category::EnvironmentFile
        | Category::ProjectWorkspace
        | Category::SourceDirectory
        | Category::BuildManifest
        | Category::ProjectAsset
        | Category::InstalledSoftware => {
            return false;
        }
        _ => {}
    }

    // If ownership says this is a project root or configuration, it's unsafe
    if let Some(om) = ownership {
        match om.evidence.ownership_type {
            ownership::OwnershipType::ProjectRoot | ownership::OwnershipType::Configuration => {
                return false;
            }
            ownership::OwnershipType::ContainedInsideProject => {
                // Check if target contains source code indicators
                let pstr = target.to_string_lossy().to_lowercase();
                if pstr.ends_with("/src") || pstr.contains("/src/") {
                    return false;
                }
            }
            _ => {}
        }
    }

    // Safe categories — cache and build artifacts
    eng.category.is_cleanable()
}
