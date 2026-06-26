// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — Phase 2: Priority Scoring (0-100 with breakdown).

use crate::planner;
use std::path::Path;

use super::types::PriorityBreakdown;

/// Size score tier: 0-40 points based on reclaimable size.
///
/// Logarithmic scaling — larger sizes give diminishing returns
/// to avoid dominance of one factor.
pub(crate) fn size_score(size_bytes: u64) -> u8 {
    if size_bytes >= 5_368_709_120 {
        40 // >= 5 GB
    } else if size_bytes >= 2_147_483_648 {
        37 // >= 2 GB
    } else if size_bytes >= 1_073_741_824 {
        33 // >= 1 GB
    } else if size_bytes >= 524_288_000 {
        28 // >= 500 MB
    } else if size_bytes >= 104_857_600 {
        22 // >= 100 MB
    } else if size_bytes >= 52_428_800 {
        17 // >= 50 MB
    } else if size_bytes >= 10_485_760 {
        12 // >= 10 MB
    } else if size_bytes >= 1_048_576 {
        6 // >= 1 MB
    } else {
        0
    }
}

/// Regenerability score: 0-25 points.
///
/// Higher when the planner confirms regeneration is possible and
/// provides a clear regeneration command.
pub(crate) fn regenerable_score(plan: &planner::CleanupPlan) -> u8 {
    if !plan.regeneration.is_empty() && plan.safe_to_clean {
        25 // Full regeneration info + safe
    } else if plan.safe_to_clean {
        15 // Safe but no explicit regeneration info
    } else {
        0
    }
}

/// Ecosystem command score: 0-20 points.
///
/// Having a native ecosystem command (cargo clean, npm install, etc.)
/// makes cleanup safer and more convenient than raw deletion.
pub(crate) fn ecosystem_score(plan: &planner::CleanupPlan) -> u8 {
    if !plan.suggested_commands.is_empty() {
        20 // Has at least one ecosystem command
    } else {
        0
    }
}

/// Confidence score: 0-15 points.
///
/// Derived from ownership detection confidence.
pub(crate) fn confidence_score(path: &Path) -> u8 {
    match crate::ownership::detect_project_ownership(path) {
        Some(om) => {
            // Scale 0-100 confidence to 0-15
            ((om.evidence.confidence as u32 * 15) / 100) as u8
        }
        None => 5, // No ownership detected — modest default
    }
}

/// Compute full priority breakdown for an opportunity.
pub(crate) fn compute_priority(
    size_bytes: u64,
    plan: &planner::CleanupPlan,
    path: &Path,
) -> PriorityBreakdown {
    let size_points = size_score(size_bytes);
    let regenerable_points = regenerable_score(plan);
    let ecosystem_points = ecosystem_score(plan);
    let confidence_points = confidence_score(path);

    let total = (size_points as u16
        + regenerable_points as u16
        + ecosystem_points as u16
        + confidence_points as u16)
        .min(100) as u8;

    PriorityBreakdown {
        size_points,
        regenerable_points,
        ecosystem_points,
        confidence_points,
        total,
    }
}
