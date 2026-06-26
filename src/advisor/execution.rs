// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — Phase 3: Time Estimation & Main Entry Point.

use crate::discovery::{self, Ecosystem};
use crate::planner;
use std::path::Path;

use super::types::{CleanupAdvisor, CleanupOpportunity};

/// Estimate regeneration time based on ecosystem and size.
///
/// Returns "Instant" for negligible sizes.
pub(crate) fn estimate_regen_time(ecosystem: Option<Ecosystem>, size_bytes: u64) -> String {
    if size_bytes < 1_048_576 {
        return "Instant".to_string();
    }

    match ecosystem {
        Some(Ecosystem::Rust) => {
            if size_bytes >= 1_073_741_824 {
                "5-15 min"
            } else if size_bytes >= 524_288_000 {
                "2-5 min"
            } else if size_bytes >= 104_857_600 {
                "1-3 min"
            } else {
                "30-60 sec"
            }
        }
        Some(Ecosystem::Node) => {
            if size_bytes >= 1_073_741_824 {
                "3-8 min"
            } else if size_bytes >= 524_288_000 {
                "1-3 min"
            } else {
                "20-60 sec"
            }
        }
        Some(Ecosystem::Python) => {
            if size_bytes >= 524_288_000 {
                "1-3 min"
            } else {
                "10-30 sec"
            }
        }
        Some(Ecosystem::Go) => {
            if size_bytes >= 1_073_741_824 {
                "2-5 min"
            } else {
                "10-60 sec"
            }
        }
        None => {
            if size_bytes >= 104_857_600 {
                "< 1 min"
            } else {
                "< 10 sec"
            }
        }
    }
    .to_string()
}

/// Estimate cleanup execution time — how long the deletion itself takes.
/// Separate from regeneration time.
///
/// Ecosystem commands (cargo clean, npm install) are near-instant
/// because they use tool-native deletion. Raw directory removal scales
/// with size and file count.
pub(crate) fn estimate_cleanup_time(action: &str, size_bytes: u64) -> String {
    // Ecosystem commands are fast — the tool handles deletion efficiently
    let is_ecosystem_cmd = !action.is_empty()
        && action != "Manual cleanup"
        && !action.starts_with("rm ")
        && !action.starts_with("find ");

    if is_ecosystem_cmd {
        if size_bytes >= 1_073_741_824 {
            "2-5 seconds"
        } else {
            "Instant"
        }
    } else if size_bytes >= 1_073_741_824 {
        "5-15 seconds"
    } else if size_bytes >= 104_857_600 {
        "1-3 seconds"
    } else {
        "Instant"
    }
    .to_string()
}

/// Compute directory size recursively.
pub(crate) fn dir_size(path: &Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }
    walkdir_size(path)
}

pub(crate) fn walkdir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                total += walkdir_size(&entry.path());
            } else if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }
    }
    total
}

/// Minimum size (bytes) to be worth showing as a cleanup opportunity.
const MINIMUM_MEANINGFUL_SIZE: u64 = 1_048_576; // 1 MB

/// Run the cleanup advisor on a directory.
///
/// Discovers all cleanable opportunities, scores them, deduplicates,
/// groups by action, and returns a ranked advisor result.
/// Returns an empty advisor if no opportunities are found (caller should
/// fall back to single-path planner).
pub fn advise(root: &Path) -> CleanupAdvisor {
    let project = discovery::find_project_for_path(root);
    let ecosystem = project.as_ref().map(|p| p.ecosystem);

    let project_name = project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| {
        root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    let candidates = super::discover::discover_candidates(root, ecosystem);
    let mut opportunities: Vec<CleanupOpportunity> = Vec::new();

    for candidate_path in &candidates {
        // Skip dangerous paths
        if planner::check_path_blocked(candidate_path).is_err() {
            continue;
        }

        // Use existing planner for full analysis — NO duplicated logic
        let plan = planner::plan(candidate_path);

        // Only include safe-to-clean items
        if !plan.safe_to_clean {
            continue;
        }

        // Skip items below minimum meaningful size
        if plan.estimated_reclaimable_bytes < MINIMUM_MEANINGFUL_SIZE {
            continue;
        }

        // Determine display name (relative to root)
        let display_name = candidate_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Determine the best action from planner
        let planner_action = if !plan.suggested_commands.is_empty() {
            plan.suggested_commands[0].clone()
        } else if !plan.recommendation.is_empty() {
            plan.recommendation.clone()
        } else {
            "Manual cleanup".to_string()
        };

        // Ecosystem-aware action override
        let (action, was_overridden) = super::discover::ecosystem_action_override(
            &display_name,
            ecosystem,
            project.as_ref(),
            &planner_action,
        );

        // Determine reason
        let reason = if !plan.reason.is_empty() {
            plan.reason.clone()
        } else if !plan.regeneration.is_empty() {
            plan.regeneration.clone()
        } else {
            "Reclaimable disk space.".to_string()
        };

        // Recompute ecosystem score if we overrode the action
        let mut priority = super::scoring::compute_priority(
            plan.estimated_reclaimable_bytes,
            &plan,
            candidate_path,
        );
        if was_overridden && priority.ecosystem_points == 0 {
            priority.ecosystem_points = 20;
            priority.total = (priority.total as u16 + 20).min(100) as u8;
        }

        let estimated_regen_time = estimate_regen_time(ecosystem, plan.estimated_reclaimable_bytes);

        // Collect evidence files for auditable confidence
        let evidence_files = crate::ownership::detect_project_ownership(candidate_path)
            .map(|om| om.evidence.evidence_files)
            .unwrap_or_default();

        opportunities.push(CleanupOpportunity {
            display_name: format!("{}/", display_name),
            path: candidate_path.clone(),
            size_bytes: plan.estimated_reclaimable_bytes,
            safe_to_clean: true,
            action,
            reason,
            priority,
            estimated_regen_time,
            rank: 0,
            evidence_files,
        });
    }

    // Dedup parent-child
    super::grouping::dedup_parent_child(&mut opportunities);

    // Sort: by total score descending, then by size descending
    opportunities.sort_by(|a, b| {
        b.priority
            .total
            .cmp(&a.priority.total)
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
    });

    // Set 1-based rank
    for (i, opp) in opportunities.iter_mut().enumerate() {
        opp.rank = i + 1;
    }

    let total_reclaimable: u64 = opportunities.iter().map(|o| o.size_bytes).sum();
    let directory_size = dir_size(root);

    // v8.5: Group opportunities by shared action
    let groups = super::grouping::group_opportunities(&opportunities, ecosystem);

    CleanupAdvisor {
        project_name,
        ecosystem,
        opportunities,
        groups,
        total_reclaimable,
        directory_size,
    }
}
