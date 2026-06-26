// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — v8.5.0
//!
//! Intelligent Recommendation Engine.
//! Transforms Zacxiom from a filesystem classifier into an intelligent
//! cleanup advisor that produces professional, decision-centric output.
//!
//! CRITICAL: This module NEVER deletes anything.
//! No filesystem mutations. No `rm`. Recommendation only.
//!
//! This is an ORCHESTRATION layer — it reuses:
//!   - Discovery (v8.0) for ecosystem detection
//!   - Ownership (v8.1) for project context and confidence
//!   - Impact (v8.2) for consequence analysis
//!   - Planner (v8.3) for per-path safety and recommendations
//!
//! No duplicated classification rules. No duplicated logic.
//!
//! v8.5: Grouped recommendations, action-first value cards,
//! human-friendly priority levels, execution cost estimation,
//! and full explainability for every recommendation.

use crate::discovery::Ecosystem;

// ═══════════════════════════════════════════════════════════════
// Data Structures
// ═══════════════════════════════════════════════════════════════

/// Human-friendly priority level.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityLevel {
    /// Score 80-100: Largest, safest, most impactful cleanup.
    Immediate,
    /// Score 60-79: High value, fully regenerable.
    High,
    /// Score 40-59: Moderate value, worth considering.
    Medium,
    /// Score 0-39: Low priority, cleanup if needed.
    Low,
}

impl PriorityLevel {
    pub(crate) fn from_score(score: u8) -> Self {
        match score {
            80..=u8::MAX => PriorityLevel::Immediate,
            60..=79 => PriorityLevel::High,
            40..=59 => PriorityLevel::Medium,
            _ => PriorityLevel::Low,
        }
    }

    pub(crate) fn display(&self) -> &'static str {
        match self {
            PriorityLevel::Immediate => "Immediate",
            PriorityLevel::High => "High",
            PriorityLevel::Medium => "Medium",
            PriorityLevel::Low => "Low",
        }
    }
}

/// Component breakdown of a priority score.
///
/// All scores are deterministic — same inputs always produce same output.
/// Total is capped at 100.
#[derive(Debug, Clone)]
pub struct PriorityBreakdown {
    /// Score contribution from size (0-40).
    pub size_points: u8,
    /// Score contribution from regenerability (0-25).
    pub regenerable_points: u8,
    /// Score contribution from ecosystem command availability (0-20).
    pub ecosystem_points: u8,
    /// Score contribution from ownership confidence (0-15).
    pub confidence_points: u8,
    /// Total priority score 0-100.
    pub total: u8,
}

impl PriorityBreakdown {
    /// Star rating 1-5 derived from total score.
    pub fn stars(&self) -> u8 {
        match self.total {
            0..=19 => 1,
            20..=39 => 2,
            40..=59 => 3,
            60..=79 => 4,
            80..=u8::MAX => 5,
        }
    }

    /// Human-friendly priority level.
    pub fn level(&self) -> PriorityLevel {
        PriorityLevel::from_score(self.total)
    }
}

/// A single cleanup opportunity discovered inside a directory.
#[derive(Debug, Clone)]
pub struct CleanupOpportunity {
    /// Display name (e.g. "target/").
    pub display_name: String,
    /// Full filesystem path.
    pub path: std::path::PathBuf,
    /// Estimated reclaimable size in bytes.
    pub size_bytes: u64,
    /// Whether the planner considers this safe to clean.
    pub safe_to_clean: bool,
    /// Ecosystem-aware cleanup command (e.g. "cargo clean").
    pub action: String,
    /// Why this is safe to clean.
    pub reason: String,
    /// Priority score with component breakdown.
    pub priority: PriorityBreakdown,
    /// Estimated time to regenerate after cleanup.
    pub estimated_regen_time: String,
    /// 1-based rank after sorting (set during final ranking).
    pub rank: usize,
    /// Evidence files supporting the confidence score.
    pub evidence_files: Vec<String>,
}

/// Execution cost — how long the cleanup action itself takes.
/// Separate from regeneration time.
#[derive(Debug, Clone)]
pub struct ExecutionCost {
    /// Human-readable cleanup time (e.g. "2-5 seconds").
    pub cleanup_time: String,
    /// Human-readable regeneration time (e.g. "2-5 minutes").
    pub regeneration_time: String,
}

/// A group of cleanup opportunities sharing the same logical action.
///
/// Instead of listing `target/`, `criterion/`, `coverage/` separately,
/// they become one group: "Rust Build Artifacts" → `cargo clean` → 762 MB.
#[derive(Debug, Clone)]
pub struct CleanupGroup {
    /// Human-readable group label (e.g. "Rust Build Artifacts").
    pub label: String,
    /// Shared cleanup action command (e.g. "cargo clean").
    pub action: String,
    /// Individual opportunities in this group.
    pub items: Vec<String>,
    /// Total reclaimable bytes across all items.
    pub total_size: u64,
    /// Aggregate priority (best score in group).
    pub priority: PriorityBreakdown,
    /// Human-friendly priority level.
    pub priority_level: PriorityLevel,
    /// Execution and regeneration cost.
    pub execution: ExecutionCost,
    /// Deduplicated reasons explaining why cleanup is safe.
    pub reasons: Vec<String>,
    /// Aggregate confidence percentage (highest in group).
    pub confidence_pct: u8,
    /// Explainable ranking reasons (why this group ranks here).
    pub ranking_reasons: Vec<String>,
}

/// Full advisor output for a directory.
#[derive(Debug, Clone)]
pub struct CleanupAdvisor {
    /// Project name (or directory name if no project detected).
    pub project_name: String,
    /// Detected ecosystem, if any.
    pub ecosystem: Option<Ecosystem>,
    /// All discovered cleanup opportunities, sorted by priority descending.
    pub opportunities: Vec<CleanupOpportunity>,
    /// Grouped recommendations (v8.5).
    pub groups: Vec<CleanupGroup>,
    /// Total estimated reclaimable bytes.
    pub total_reclaimable: u64,
    /// Total directory size (for percentage calculation).
    pub directory_size: u64,
}
