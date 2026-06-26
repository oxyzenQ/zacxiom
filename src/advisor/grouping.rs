// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — Phase 4: Parent-Child Deduplication & Phase 6: Grouping.

use crate::discovery::Ecosystem;
use crate::display::human_size;
use std::collections::HashSet;

use super::types::{
    CleanupGroup, CleanupOpportunity, ExecutionCost, PriorityBreakdown, PriorityLevel,
};

/// Remove child paths that are already covered by a parent path.
///
/// E.g. if "target/" is in the list, remove "target/debug" and
/// "target/doc" — they are subsumed by the parent.
pub(crate) fn dedup_parent_child(opportunities: &mut Vec<CleanupOpportunity>) {
    let parent_paths: HashSet<String> = opportunities
        .iter()
        .map(|o| o.path.to_string_lossy().to_string())
        .collect();

    opportunities.retain(|opp| {
        let opp_str = opp.path.to_string_lossy().to_string();
        let has_parent = parent_paths.iter().any(|parent| {
            parent != &opp_str
                && (opp_str.starts_with(&format!("{}/", parent))
                    || opp_str.starts_with(&format!("{parent}/")))
        });
        !has_parent
    });
}

/// Derive a human-friendly group label from the action command.
///
/// Maps ecosystem commands to descriptive labels. Falls back to
/// the action itself for unknown commands.
pub(crate) fn derive_group_label(action: &str, ecosystem: Option<Ecosystem>) -> String {
    match (ecosystem, action) {
        (_, a) if a.contains("cargo clean") => "Rust Build Artifacts".to_string(),
        (Some(Ecosystem::Node), a) if a.ends_with("install") => "Node Dependencies".to_string(),
        (Some(Ecosystem::Node), a) if a.contains("run build") || a.contains(" build") => {
            "Build Output".to_string()
        }
        (Some(Ecosystem::Python), a) if a.contains("pip install") => {
            "Python Environment".to_string()
        }
        (Some(Ecosystem::Python), a) if a.contains("venv") => {
            "Python Virtual Environment".to_string()
        }
        (Some(Ecosystem::Python), _) => "Python Cache".to_string(),
        (Some(Ecosystem::Go), a) if a.contains("go clean") => "Go Build Cache".to_string(),
        _ => {
            // Fallback: capitalize first letter of action
            let mut label = action.to_string();
            if let Some(first) = label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}

/// Build explainable ranking reasons for a group.
///
/// Answers "Why does this group rank here?" with human-readable bullets.
pub(crate) fn ranking_reasons(group: &CleanupGroup) -> Vec<String> {
    let mut reasons = Vec::new();

    // Size reasoning
    if group.total_size >= 1_073_741_824 {
        reasons.push(format!(
            "Reclaims {} of disk space",
            human_size(group.total_size)
        ));
    } else if group.total_size >= 104_857_600 {
        reasons.push(format!(
            "Reclaims {} of recoverable space",
            human_size(group.total_size)
        ));
    }

    // Regenerability reasoning
    if group.priority.regenerable_points >= 25 {
        reasons.push("Fully regenerable from source".to_string());
    } else if group.priority.regenerable_points >= 15 {
        reasons.push("Safe to remove".to_string());
    }

    // Ecosystem command reasoning
    if group.priority.ecosystem_points > 0 {
        reasons.push("Official ecosystem cleanup command".to_string());
    }

    // Confidence reasoning
    if group.confidence_pct >= 80 {
        reasons.push("High ownership confidence".to_string());
    }

    // Multiple items grouped together
    if group.items.len() > 1 {
        reasons.push(format!("Covers {} related artifacts", group.items.len()));
    }

    // Low execution cost
    if group.execution.cleanup_time == "Instant" {
        reasons.push("Instant cleanup execution".to_string());
    }

    reasons
}

/// Group individual opportunities by their shared cleanup action.
///
/// Opportunities with the same action command are merged into a single
/// `CleanupGroup` with aggregated size and the best priority score.
pub(crate) fn group_opportunities(
    opportunities: &[CleanupOpportunity],
    ecosystem: Option<Ecosystem>,
) -> Vec<CleanupGroup> {
    let mut action_groups: std::collections::HashMap<String, Vec<&CleanupOpportunity>> =
        std::collections::HashMap::new();

    // Group by action command
    for opp in opportunities {
        let key: String = if !opp.action.is_empty() && opp.action != "Manual cleanup" {
            opp.action.clone()
        } else {
            opp.display_name.clone()
        };
        action_groups.entry(key).or_default().push(opp);
    }

    let mut groups: Vec<CleanupGroup> = action_groups
        .into_iter()
        .map(|(action, opps)| {
            let total_size: u64 = opps.iter().map(|o| o.size_bytes).sum();
            let best_priority = opps
                .iter()
                .max_by_key(|o| o.priority.total)
                .map(|o| o.priority.clone())
                .unwrap_or(PriorityBreakdown {
                    size_points: 0,
                    regenerable_points: 0,
                    ecosystem_points: 0,
                    confidence_points: 0,
                    total: 0,
                });
            let best_confidence = opps
                .iter()
                .max_by_key(|o| o.priority.confidence_points)
                .map(|o| (o.priority.confidence_points as u16 * 100 / 15) as u8)
                .unwrap_or(0);

            // Deduplicated reasons
            let mut reasons: Vec<String> = opps
                .iter()
                .map(|o| o.reason.clone())
                .filter(|r| !r.is_empty())
                .collect();
            reasons.dedup();

            // Items as display names
            let items: Vec<String> = opps.iter().map(|o| o.display_name.clone()).collect();

            // Time estimates use the largest item's values
            let largest = opps.iter().max_by_key(|o| o.size_bytes).unwrap();

            let label = derive_group_label(&action, ecosystem);
            let priority_level = PriorityLevel::from_score(best_priority.total);

            let mut group = CleanupGroup {
                label,
                action: action.clone(),
                items,
                total_size,
                priority: best_priority,
                priority_level,
                execution: ExecutionCost {
                    cleanup_time: super::execution::estimate_cleanup_time(&action, total_size),
                    regeneration_time: largest.estimated_regen_time.clone(),
                },
                reasons,
                confidence_pct: best_confidence,
                ranking_reasons: Vec::new(), // Set below
            };

            group.ranking_reasons = ranking_reasons(&group);
            group
        })
        .collect();

    // Sort groups by: priority total desc, then size desc
    groups.sort_by(|a, b| {
        b.priority
            .total
            .cmp(&a.priority.total)
            .then_with(|| b.total_size.cmp(&a.total_size))
    });

    groups
}
