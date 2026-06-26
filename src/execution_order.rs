// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Execution Ordering — v8.6
//!
//! Deterministic execution order for cleanup operations.
//! Prioritizes by risk, reclaim, rebuild cost, and dependencies.
//!
//! The ordering algorithm is deterministic: same inputs always produce
//! the same order.  No randomness, no heuristics that vary by machine state.

use crate::engine::types::RiskLevel;
use crate::planner::CleanupPlan;

/// Priority score breakdown for a cleanup plan.
///
/// Each component contributes to the total priority score (0-100).
/// Higher score = clean first (safer, more impactful).
#[derive(Debug, Clone)]
pub struct ExecutionPriority {
    /// Score from safety/risk (0-40). Safer = higher score.
    pub safety_score: u8,
    /// Score from reclaimable size (0-30). More space = higher score.
    pub reclaim_score: u8,
    /// Score from rebuild cost (0-20). Cheaper rebuild = higher score.
    pub rebuild_score: u8,
    /// Score from dependency chain (0-10). Fewer deps = higher score.
    pub dependency_score: u8,
    /// Total priority score (0-100).
    pub total: u8,
}

/// Ordered plan with execution priority metadata.
#[derive(Debug, Clone)]
pub struct OrderedPlan {
    pub plan: CleanupPlan,
    pub priority: ExecutionPriority,
    /// Path this plan corresponds to.
    pub path: String,
}

/// Compute execution priority for a cleanup plan.
///
/// Weighting rationale:
///   Safety   (40%): Never risk data loss — safest items go first.
///   Reclaim  (30%): Maximize impact — largest reclaims go first.
///   Rebuild  (20%): Cheapest rebuilds first — low-hanging fruit.
///   Deps     (10%): Items with fewer dependencies first.
pub fn compute_priority(plan: &CleanupPlan, path: &str) -> ExecutionPriority {
    // ── Safety score (0-40) ──
    let safety_score = match plan.risk_level {
        RiskLevel::Minimal => 40,
        RiskLevel::Low => 35,
        RiskLevel::Moderate => 20,
        RiskLevel::High => 10,
        RiskLevel::Critical => 0,
    };

    // ── Reclaim score (0-30) ──
    let reclaim_score = reclaim_to_score(plan.estimated_reclaimable_bytes);

    // ── Rebuild score (0-20) ──
    let rebuild_score = rebuild_to_score(&plan.regeneration, plan.estimated_reclaimable_bytes);

    // ── Dependency score (0-10) ──
    let dependency_score = dependency_to_score(&plan.suggested_commands, path);

    let total = (safety_score + reclaim_score + rebuild_score + dependency_score).min(100);

    ExecutionPriority {
        safety_score,
        reclaim_score,
        rebuild_score,
        dependency_score,
        total,
    }
}

/// Convert reclaimable bytes to a 0-30 score.
///
/// Logarithmic scale so that differences at smaller sizes are visible
/// but large differences at GB scale don't overwhelm the score.
fn reclaim_to_score(bytes: u64) -> u8 {
    if bytes == 0 {
        return 0;
    }
    // Log2-based scoring: 1 byte = 1 point, ~1 GB = 30 points
    let log2 = (bytes as f64).log2();
    // log2(1) = 0, log2(1GB) ≈ 30
    (log2.min(30.0) as u8).min(30)
}

/// Convert rebuild cost to a 0-20 score.
///
/// Cheaper/faster rebuild = higher score (clean first).
/// Items that are "Automatic" or "Instant" score highest.
fn rebuild_to_score(regeneration: &str, size_bytes: u64) -> u8 {
    let regen_lower = regeneration.to_lowercase();

    // Fast rebuilds score highest
    if regen_lower.contains("automatic") || regen_lower.contains("instant") {
        return 20;
    }

    // Ecosystem commands with known cost
    if regen_lower.contains("cargo") {
        // Rust rebuilds are fast for small projects, slow for large
        if size_bytes < 1_048_576 {
            return 18;
        }
        return 14;
    }
    if regen_lower.contains("npm") || regen_lower.contains("pnpm") || regen_lower.contains("yarn") {
        if size_bytes < 10_485_760 {
            return 16;
        }
        return 12;
    }
    if regen_lower.contains("pip") || regen_lower.contains("venv") {
        return 15;
    }
    if regen_lower.contains("go") {
        return 18;
    }
    if regen_lower.contains("build") || regen_lower.contains("compile") {
        return 10;
    }

    // Manual rebuild = lower score
    if regen_lower.contains("reinstall") || regen_lower.contains("manual") {
        return 5;
    }
    if regen_lower.contains("irreplaceable") || regen_lower.contains("cannot be regenerated") {
        return 0;
    }

    // Neutral default
    8
}

/// Score based on dependency count.
///
/// Fewer dependencies = easier to clean safely = higher score.
fn dependency_to_score(commands: &[String], _path: &str) -> u8 {
    if commands.is_empty() {
        return 10; // No dependencies, fully standalone
    }
    // More commands = more dependencies
    let count = commands.len().min(10) as u8;
    10u8.saturating_sub(count)
}

/// Order cleanup plans by execution priority.
///
/// Sort is deterministic: total score desc, then reclaim desc, then path asc.
pub fn order_plans(plans: Vec<(CleanupPlan, String)>) -> Vec<OrderedPlan> {
    let mut ordered: Vec<OrderedPlan> = plans
        .into_iter()
        .map(|(plan, path)| {
            let priority = compute_priority(&plan, &path);
            OrderedPlan {
                plan,
                priority,
                path,
            }
        })
        .collect();

    ordered.sort_by(|a, b| {
        b.priority
            .total
            .cmp(&a.priority.total)
            .then_with(|| {
                b.plan
                    .estimated_reclaimable_bytes
                    .cmp(&a.plan.estimated_reclaimable_bytes)
            })
            .then_with(|| a.path.cmp(&b.path))
    });

    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::RiskLevel;

    fn test_plan(
        reclaim_bytes: u64,
        risk: RiskLevel,
        regeneration: &str,
        commands: Vec<String>,
    ) -> CleanupPlan {
        CleanupPlan {
            safe_to_clean: true,
            risk_level: risk,
            estimated_reclaimable_bytes: reclaim_bytes,
            recommendation: String::new(),
            reason: String::new(),
            regeneration: regeneration.to_string(),
            suggested_commands: commands,
            notes: vec![],
            safer_alternatives: vec![],
            expected_result: String::new(),
        }
    }

    #[test]
    fn test_safest_items_first() {
        let safe = test_plan(1000, RiskLevel::Low, "Automatic", vec![]);
        let risky = test_plan(1000, RiskLevel::High, "Automatic", vec![]);

        let plans = vec![
            (risky, "/tmp/risky".to_string()),
            (safe, "/tmp/safe".to_string()),
        ];
        let ordered = order_plans(plans);

        assert_eq!(ordered[0].path, "/tmp/safe");
        assert_eq!(ordered[1].path, "/tmp/risky");
    }

    #[test]
    fn test_larger_reclaims_first() {
        let small = test_plan(100, RiskLevel::Low, "Automatic", vec![]);
        let large = test_plan(1_000_000, RiskLevel::Low, "Automatic", vec![]);

        let plans = vec![
            (small, "/tmp/small".to_string()),
            (large, "/tmp/large".to_string()),
        ];
        let ordered = order_plans(plans);

        assert_eq!(ordered[0].path, "/tmp/large");
    }

    #[test]
    fn test_cheaper_rebuild_first() {
        let cheap = test_plan(1000, RiskLevel::Low, "Automatic", vec![]);
        let expensive = test_plan(1000, RiskLevel::Low, "cargo build --release", vec![]);

        let auto_priority = compute_priority(&cheap, "/tmp/cheap");
        let cargo_priority = compute_priority(&expensive, "/tmp/expensive");
        assert!(auto_priority.rebuild_score > cargo_priority.rebuild_score);
    }

    #[test]
    fn test_deterministic_output() {
        let a = test_plan(500, RiskLevel::Low, "Automatic", vec![]);
        let b = test_plan(500, RiskLevel::Low, "Automatic", vec![]);

        let ordered1 = order_plans(vec![
            (a.clone(), "/tmp/a".to_string()),
            (b.clone(), "/tmp/b".to_string()),
        ]);
        let ordered2 = order_plans(vec![(b, "/tmp/b".to_string()), (a, "/tmp/a".to_string())]);

        assert_eq!(ordered1[0].path, ordered2[0].path);
        assert_eq!(ordered1[1].path, ordered2[1].path);
    }

    #[test]
    fn test_reclaim_score_log_scale() {
        let tiny = reclaim_to_score(1);
        let kb = reclaim_to_score(1024);
        let mb = reclaim_to_score(1_048_576);
        let gb = reclaim_to_score(1_073_741_824);

        assert!(tiny < kb);
        assert!(kb < mb);
        assert!(mb < gb);
        assert!(gb <= 30);
    }
}
