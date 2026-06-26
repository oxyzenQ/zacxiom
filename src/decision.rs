// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Decision Intelligence — v9
//!
//! Context-aware recommendation engine.
//! Combines risk, reclaim, usage, age, rebuild cost, and confidence
//! into a single, explainable decision score.
//!
//! No opaque decisions. Every score component is auditable.
//!
//! Architecture:
//!   analyze() → DecisionFactors → compute_score() → CleanupDecision
//!
//! The decision engine is stateless and deterministic.

use crate::engine::types::RiskLevel;
use crate::planner::CleanupPlan;
use std::path::Path;
use std::time::SystemTime;

/// Weighted decision factors.
///
/// Each factor contributes to the final decision score (0-100).
/// Higher score = stronger recommendation to clean.
#[derive(Debug, Clone)]
pub struct DecisionFactors {
    /// Risk score (0-25): Lower risk = higher score.
    /// Derived from engine classification risk level.
    pub risk: u8,
    /// Reclaim score (0-25): More space = higher score.
    /// Derived from estimated_reclaimable_bytes (logarithmic).
    pub reclaim: u8,
    /// Usage score (0-15): Stale/unused = higher score.
    /// Derived from file age and access patterns.
    pub usage: u8,
    /// Age score (0-15): Older = higher score to clean.
    /// Derived from last modification time.
    pub age: u8,
    /// Rebuild cost score (0-15): Cheaper rebuild = higher score.
    /// Derived from ecosystem rebuild estimation.
    pub rebuild_cost: u8,
    /// Confidence score (0-5): Higher classification confidence = higher score.
    /// Derived from engine confidence percentage.
    pub confidence: u8,
}

/// Cleanup decision with explainable reasoning.
#[derive(Debug, Clone)]
pub struct CleanupDecision {
    /// Total decision score (0-100).
    pub score: u8,
    /// Individual factor scores.
    pub factors: DecisionFactors,
    /// Human-readable recommendation (CLEAN / REVIEW / KEEP).
    pub recommendation: DecisionRecommendation,
    /// Explainable reasoning for each factor.
    pub explanations: Vec<String>,
    /// Path being decided on.
    pub path: String,
}

/// Decision recommendation level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionRecommendation {
    /// Score >= 70: Strongly recommend cleaning.
    Clean,
    /// Score 40-69: Review before cleaning.
    Review,
    /// Score < 40: Keep — do not clean.
    Keep,
}

impl DecisionRecommendation {
    pub fn display(&self) -> &'static str {
        match self {
            DecisionRecommendation::Clean => "CLEAN",
            DecisionRecommendation::Review => "REVIEW",
            DecisionRecommendation::Keep => "KEEP",
        }
    }

    pub fn is_cleanable(&self) -> bool {
        matches!(
            self,
            DecisionRecommendation::Clean | DecisionRecommendation::Review
        )
    }
}

/// Analyze file metadata and plan to produce decision factors.
pub fn analyze(path: &Path, plan: &CleanupPlan) -> DecisionFactors {
    let risk = risk_to_score(plan);
    let reclaim = reclaim_to_score(plan.estimated_reclaimable_bytes);
    let (usage, age) = analyze_file_metadata(path);
    let rebuild_cost = rebuild_to_score(&plan.regeneration);
    let confidence = confidence_to_score(plan);

    DecisionFactors {
        risk,
        reclaim,
        usage,
        age,
        rebuild_cost,
        confidence,
    }
}

/// Compute the weighted decision score from factors.
///
/// Weight distribution:
///   Risk:        25% — safety is paramount
///   Reclaim:     25% — maximize disk space recovery
///   Usage:       15% — prefer cleaning stale/unused data
///   Age:         15% — prefer cleaning older artifacts
///   Rebuild:     15% — prefer cheaper rebuilds
///   Confidence:   5% — trust high-confidence classifications more
pub fn compute_score(factors: &DecisionFactors) -> u8 {
    (factors.risk
        + factors.reclaim
        + factors.usage
        + factors.age
        + factors.rebuild_cost
        + factors.confidence)
        .min(100)
}

/// Produce a full cleanup decision with explanations.
pub fn decide(path: &Path, plan: &CleanupPlan) -> CleanupDecision {
    let factors = analyze(path, plan);
    let score = compute_score(&factors);
    let recommendation = match score {
        70..=100 => DecisionRecommendation::Clean,
        40..=69 => DecisionRecommendation::Review,
        _ => DecisionRecommendation::Keep,
    };

    let mut explanations = Vec::new();
    explanations.push(format!(
        "Risk: {}/25 — {}",
        factors.risk,
        risk_explanation(plan)
    ));
    explanations.push(format!(
        "Reclaim: {}/25 — {}",
        factors.reclaim,
        crate::display::human_size(plan.estimated_reclaimable_bytes)
    ));
    if factors.usage > 10 {
        explanations.push(format!(
            "Usage: {}/15 — file appears stale or infrequently accessed",
            factors.usage
        ));
    } else {
        explanations.push(format!(
            "Usage: {}/15 — file appears actively used",
            factors.usage
        ));
    }
    if factors.age > 10 {
        explanations.push(format!(
            "Age: {}/15 — artifact is old (low probability of recent need)",
            factors.age
        ));
    } else {
        explanations.push(format!(
            "Age: {}/15 — artifact is relatively new",
            factors.age
        ));
    }
    explanations.push(format!(
        "Rebuild: {}/15 — {}",
        factors.rebuild_cost,
        if plan.regeneration.is_empty() {
            "not regenerable"
        } else {
            &plan.regeneration
        }
    ));
    explanations.push(format!("Confidence: {}/5", factors.confidence));

    CleanupDecision {
        score,
        factors,
        recommendation,
        explanations,
        path: path.to_string_lossy().to_string(),
    }
}

/// Convert risk level to score (0-25).
fn risk_to_score(plan: &CleanupPlan) -> u8 {
    match plan.risk_level {
        RiskLevel::Minimal => 25,
        RiskLevel::Low => 20,
        RiskLevel::Moderate => 12,
        RiskLevel::High => 5,
        RiskLevel::Critical => 0,
    }
}

/// Convert reclaimable bytes to score (0-25).
fn reclaim_to_score(bytes: u64) -> u8 {
    if bytes == 0 {
        return 0;
    }
    // Log scale: 1 byte = 1, 1 TB = 25
    let log2 = (bytes as f64).log2();
    (log2.min(40.0) * 25.0 / 40.0) as u8
}

/// Analyze file metadata for usage and age scores.
///
/// Returns (usage_score, age_score) each 0-15.
fn analyze_file_metadata(path: &Path) -> (u8, u8) {
    let meta = std::fs::symlink_metadata(path);

    match meta {
        Ok(m) => {
            // Age: time since last modification
            let age_score = match m.modified() {
                Ok(mod_time) => {
                    let elapsed = SystemTime::now()
                        .duration_since(mod_time)
                        .unwrap_or_default();
                    let days = elapsed.as_secs() / 86400;
                    // Older = higher score to clean
                    if days > 365 {
                        15 // >1 year
                    } else if days > 180 {
                        12
                    } else if days > 90 {
                        9
                    } else if days > 30 {
                        6
                    } else if days > 7 {
                        3
                    } else {
                        1 // <1 week — probably active
                    }
                }
                Err(_) => 8, // Can't determine — assume moderate
            };

            // Usage: time since last access
            let usage_score = {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let atime = m.atime();
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let elapsed_days = ((now - atime) / 86400).max(0);
                    // Stale access = higher score to clean
                    if elapsed_days > 365 {
                        15
                    } else if elapsed_days > 90 {
                        12
                    } else if elapsed_days > 30 {
                        8
                    } else if elapsed_days > 7 {
                        4
                    } else {
                        2 // Accessed recently — probably in use
                    }
                }
                #[cfg(not(unix))]
                {
                    8 // Non-Unix platform — assume moderate usage
                }
            };

            (usage_score, age_score)
        }
        Err(_) => (0, 0), // Can't determine — assume active
    }
}

/// Convert regeneration description to rebuild cost score (0-15).
fn rebuild_to_score(regeneration: &str) -> u8 {
    let lower = regeneration.to_lowercase();
    if lower.is_empty() {
        return 15; // Not regenerable = don't penalize (max score)
    }
    if lower.contains("automatic") || lower.contains("instant") {
        return 15;
    }
    if lower.contains("cargo") || lower.contains("go build") || lower.contains("zig build") {
        return 12;
    }
    if lower.contains("npm")
        || lower.contains("pnpm")
        || lower.contains("yarn")
        || lower.contains("pip")
    {
        return 10;
    }
    if lower.contains("build") || lower.contains("compile") {
        return 8;
    }
    if lower.contains("reinstall") || lower.contains("manual") {
        return 4;
    }
    if lower.contains("irreplaceable") || lower.contains("cannot") {
        return 0;
    }
    7
}

/// Convert confidence to score (0-5).
fn confidence_to_score(plan: &CleanupPlan) -> u8 {
    // Approximate from plan quality
    if plan.reason.is_empty() {
        return 3;
    }
    if plan.reason.len() > 80 {
        5
    } else if !plan.regeneration.is_empty() {
        4
    } else {
        3
    }
}

fn risk_explanation(plan: &CleanupPlan) -> String {
    match plan.risk_level {
        RiskLevel::Minimal => "Fully regenerable cache — safe to clean",
        RiskLevel::Low => "Cache with some rebuild cost",
        RiskLevel::Moderate => "Application data — review before cleaning",
        RiskLevel::High => "User data or configuration — manual review required",
        RiskLevel::Critical => "System-critical — never clean",
    }
    .to_string()
}

/// Apply cleanup policy filtering to a set of decisions.
///
/// v9: Policy engine — configurable cleanup thresholds.
pub struct CleanupPolicy {
    /// Minimum score to auto-clean (default 70).
    pub auto_clean_threshold: u8,
    /// Minimum score to suggest review (default 40).
    pub review_threshold: u8,
    /// Maximum age in days to consider "active" (default 30).
    pub active_age_days: u64,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        CleanupPolicy {
            auto_clean_threshold: 70,
            review_threshold: 40,
            active_age_days: 30,
        }
    }
}

impl CleanupPolicy {
    /// Filter decisions through the policy.
    pub fn apply(&self, decisions: &[CleanupDecision]) -> Vec<CleanupDecision> {
        decisions
            .iter()
            .filter(|d| d.score >= self.review_threshold)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::RiskLevel;

    fn test_plan(reclaim: u64, risk: RiskLevel, regen: &str) -> CleanupPlan {
        CleanupPlan {
            safe_to_clean: true,
            risk_level: risk,
            estimated_reclaimable_bytes: reclaim,
            recommendation: String::new(),
            reason: "Test reason".to_string(),
            regeneration: regen.to_string(),
            suggested_commands: vec![],
            notes: vec![],
            safer_alternatives: vec![],
            expected_result: String::new(),
        }
    }

    #[test]
    fn test_high_score_for_large_safe_reclaim() {
        let plan = test_plan(10_737_418_240, RiskLevel::Minimal, "cargo build");
        let decision = decide(Path::new("/tmp/test"), &plan);
        assert!(decision.score >= 60);
        assert!(
            decision.recommendation == DecisionRecommendation::Review
                || decision.recommendation == DecisionRecommendation::Clean
        );
    }

    #[test]
    fn test_low_score_for_critical_risk() {
        let plan = test_plan(1_000_000, RiskLevel::Critical, "");
        let decision = decide(Path::new("/tmp/test"), &plan);
        assert!(decision.score < 40);
        assert_eq!(decision.recommendation, DecisionRecommendation::Keep);
    }

    #[test]
    fn test_review_for_moderate() {
        let plan = test_plan(10_485_760, RiskLevel::Moderate, "npm install");
        let decision = decide(Path::new("/tmp/test"), &plan);
        assert_eq!(decision.recommendation, DecisionRecommendation::Review);
    }

    #[test]
    fn test_policy_filters_below_threshold() {
        let policy = CleanupPolicy::default();
        let plan1 = test_plan(1_000, RiskLevel::High, "");
        let plan2 = test_plan(1_073_741_824, RiskLevel::Minimal, "cargo build");

        let decisions = vec![
            decide(Path::new("/tmp/a"), &plan1),
            decide(Path::new("/tmp/b"), &plan2),
        ];
        let filtered = policy.apply(&decisions);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, "/tmp/b");
    }

    #[test]
    fn test_reclaim_score_log_scale() {
        assert!(reclaim_to_score(1) < reclaim_to_score(1024));
        assert!(reclaim_to_score(1_048_576) < reclaim_to_score(1_073_741_824));
    }

    #[test]
    fn test_risk_score_ordering() {
        let minimal = test_plan(1000, RiskLevel::Minimal, "");
        let critical = test_plan(1000, RiskLevel::Critical, "");
        assert!(risk_to_score(&minimal) > risk_to_score(&critical));
    }

    #[test]
    fn test_all_explanations_produced() {
        let plan = test_plan(1_000_000, RiskLevel::Low, "cargo build");
        let decision = decide(Path::new("/tmp/test"), &plan);
        assert_eq!(decision.explanations.len(), 6);
        for exp in &decision.explanations {
            assert!(!exp.is_empty());
        }
    }
}
