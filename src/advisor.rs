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

use crate::color;
use crate::discovery::{self, Ecosystem, ProjectInfo};
use crate::display::human_size;
use crate::planner;
use std::collections::HashSet;
use std::path::Path;

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
    fn from_score(score: u8) -> Self {
        match score {
            80..=u8::MAX => PriorityLevel::Immediate,
            60..=79 => PriorityLevel::High,
            40..=59 => PriorityLevel::Medium,
            _ => PriorityLevel::Low,
        }
    }

    fn display(&self) -> &'static str {
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

// ═══════════════════════════════════════════════════════════════
// Phase 1: Cleanup Opportunity Discovery
// ═══════════════════════════════════════════════════════════════

/// Known cleanable directory names, by ecosystem.
/// The planner's classifier is the final authority — these are just
/// candidates to CHECK, not automatic inclusions.
fn ecosystem_candidates(ecosystem: Option<Ecosystem>) -> Vec<&'static str> {
    let mut candidates: Vec<&str> = vec![".cache", "tmp", "logs"];

    match ecosystem {
        Some(Ecosystem::Rust) => {
            candidates.extend_from_slice(&["target", "criterion", "coverage"]);
        }
        Some(Ecosystem::Node) => {
            candidates.extend_from_slice(&[
                "node_modules",
                "dist",
                ".next",
                ".turbo",
                ".parcel-cache",
            ]);
        }
        Some(Ecosystem::Python) => {
            candidates.extend_from_slice(&[
                "__pycache__",
                ".pytest_cache",
                ".mypy_cache",
                ".ruff_cache",
                ".tox",
                ".venv",
            ]);
        }
        Some(Ecosystem::Go) => {
            candidates.extend_from_slice(&[]);
        }
        None => {}
    }

    candidates
}

/// Discover existing cleanable candidates inside a directory.
fn discover_candidates(root: &Path, ecosystem: Option<Ecosystem>) -> Vec<std::path::PathBuf> {
    let candidates = ecosystem_candidates(ecosystem);
    let mut found = Vec::new();

    for name in &candidates {
        let candidate = root.join(name);
        if candidate.exists() {
            found.push(candidate);
        }
    }

    found
}

// ═══════════════════════════════════════════════════════════════
// Phase 2: Priority Scoring (0-100 with breakdown)
// ═══════════════════════════════════════════════════════════════

/// Size score tier: 0-40 points based on reclaimable size.
///
/// Logarithmic scaling — larger sizes give diminishing returns
/// to avoid dominance of one factor.
fn size_score(size_bytes: u64) -> u8 {
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
fn regenerable_score(plan: &planner::CleanupPlan) -> u8 {
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
fn ecosystem_score(plan: &planner::CleanupPlan) -> u8 {
    if !plan.suggested_commands.is_empty() {
        20 // Has at least one ecosystem command
    } else {
        0
    }
}

/// Confidence score: 0-15 points.
///
/// Derived from ownership detection confidence.
fn confidence_score(path: &Path) -> u8 {
    match crate::ownership::detect_project_ownership(path) {
        Some(om) => {
            // Scale 0-100 confidence to 0-15
            ((om.evidence.confidence as u32 * 15) / 100) as u8
        }
        None => 5, // No ownership detected — modest default
    }
}

/// Compute full priority breakdown for an opportunity.
fn compute_priority(
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

// ═══════════════════════════════════════════════════════════════
// Phase 3: Time Estimation
// ═══════════════════════════════════════════════════════════════

/// Estimate regeneration time based on ecosystem and size.
///
/// Returns "Instant" for negligible sizes.
fn estimate_regen_time(ecosystem: Option<Ecosystem>, size_bytes: u64) -> String {
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
fn estimate_cleanup_time(action: &str, size_bytes: u64) -> String {
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

// ═══════════════════════════════════════════════════════════════
// Phase 4: Parent-Child Deduplication
// ═══════════════════════════════════════════════════════════════

/// Remove child paths that are already covered by a parent path.
///
/// E.g. if "target/" is in the list, remove "target/debug" and
/// "target/doc" — they are subsumed by the parent.
fn dedup_parent_child(opportunities: &mut Vec<CleanupOpportunity>) {
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

// ═══════════════════════════════════════════════════════════════
// Phase 5: Ecosystem-Aware Action Override
// ═══════════════════════════════════════════════════════════════

/// Minimum size (bytes) to be worth showing as a cleanup opportunity.
const MINIMUM_MEANINGFUL_SIZE: u64 = 1_048_576; // 1 MB

/// Detect the Node.js package manager from lockfiles in the project.
fn detect_node_pm(project: &ProjectInfo) -> &'static str {
    for m in &project.manifests {
        let name = m.file_name().and_then(|n| n.to_str()).unwrap_or("");
        match name {
            "pnpm-lock.yaml" => return "pnpm",
            "yarn.lock" => return "yarn",
            "package-lock.json" => return "npm",
            _ => {}
        }
    }
    "npm" // Default fallback
}

/// Override the planner's action with an ecosystem-aware command
/// when the planner falls through to generic wording.
fn ecosystem_action_override(
    display_name: &str,
    ecosystem: Option<Ecosystem>,
    project: Option<&ProjectInfo>,
    planner_action: &str,
) -> (String, bool) {
    let is_generic = planner_action == "Remove temporary files."
        || planner_action == "Clear application cache."
        || planner_action == "Manual cleanup"
        || planner_action.is_empty();

    if !is_generic {
        return (planner_action.to_string(), false);
    }

    let name = display_name.trim_end_matches('/');

    match ecosystem {
        Some(Ecosystem::Node) => {
            let pm = project.map_or("npm", detect_node_pm);
            match name {
                "node_modules" => (format!("{pm} install"), true),
                "dist" => (format!("{pm} run build"), true),
                ".next" => ("next build".to_string(), true),
                ".turbo" => ("turbo build".to_string(), true),
                ".parcel-cache" => (format!("{pm} run build"), true),
                _ => (planner_action.to_string(), false),
            }
        }
        Some(Ecosystem::Rust) => match name {
            "target" => ("cargo clean".to_string(), true),
            "criterion" => ("cargo clean".to_string(), true),
            "coverage" => ("cargo clean".to_string(), true),
            _ => (planner_action.to_string(), false),
        },
        Some(Ecosystem::Python) => match name {
            "__pycache__" => (
                "find . -type d -name __pycache__ -exec rm -rf {} +".to_string(),
                true,
            ),
            ".pytest_cache" => ("pytest --cache-clear".to_string(), true),
            ".mypy_cache" => ("rm -rf .mypy_cache".to_string(), true),
            ".ruff_cache" => ("ruff clean".to_string(), true),
            ".venv" => ("python -m venv .venv && pip install -e .".to_string(), true),
            _ => (planner_action.to_string(), false),
        },
        Some(Ecosystem::Go) => (planner_action.to_string(), false),
        None => (planner_action.to_string(), false),
    }
}

// ═══════════════════════════════════════════════════════════════
// Phase 6: Opportunity Grouping (v8.5)
// ═══════════════════════════════════════════════════════════════

/// Derive a human-friendly group label from the action command.
///
/// Maps ecosystem commands to descriptive labels. Falls back to
/// the action itself for unknown commands.
fn derive_group_label(action: &str, ecosystem: Option<Ecosystem>) -> String {
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
fn ranking_reasons(group: &CleanupGroup) -> Vec<String> {
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
fn group_opportunities(
    opportunities: &[CleanupOpportunity],
    ecosystem: Option<Ecosystem>,
) -> Vec<CleanupGroup> {
    let mut action_groups: std::collections::HashMap<String, Vec<&CleanupOpportunity>> =
        std::collections::HashMap::new();

    // Group by action command
    for opp in opportunities {
        let key = if !opp.action.is_empty() && opp.action != "Manual cleanup" {
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
                    cleanup_time: estimate_cleanup_time(&action, total_size),
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

// ═══════════════════════════════════════════════════════════════
// Main Advisor Function
// ═══════════════════════════════════════════════════════════════

/// Compute directory size recursively.
fn dir_size(path: &Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }
    walkdir_size(path)
}

fn walkdir_size(path: &Path) -> u64 {
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

    let candidates = discover_candidates(root, ecosystem);
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
        let (action, was_overridden) =
            ecosystem_action_override(&display_name, ecosystem, project.as_ref(), &planner_action);

        // Determine reason
        let reason = if !plan.reason.is_empty() {
            plan.reason.clone()
        } else if !plan.regeneration.is_empty() {
            plan.regeneration.clone()
        } else {
            "Reclaimable disk space.".to_string()
        };

        // Recompute ecosystem score if we overrode the action
        let mut priority =
            compute_priority(plan.estimated_reclaimable_bytes, &plan, candidate_path);
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
    dedup_parent_child(&mut opportunities);

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
    let groups = group_opportunities(&opportunities, ecosystem);

    CleanupAdvisor {
        project_name,
        ecosystem,
        opportunities,
        groups,
        total_reclaimable,
        directory_size,
    }
}

// ═══════════════════════════════════════════════════════════════
// Phase 7: Rendering — Intelligent Recommendation Output (v8.5)
// ═══════════════════════════════════════════════════════════════

/// Render star rating as Unicode stars.
fn star_rating(stars: u8) -> String {
    let filled = stars as usize;
    let empty = 5_usize.saturating_sub(filled);
    format!("{}{}", "\u{2605}".repeat(filled), "\u{2606}".repeat(empty))
}

/// Circled number labels for recommended order.
fn circled_number(n: usize) -> &'static str {
    match n {
        1 => "\u{2460}",
        2 => "\u{2461}",
        3 => "\u{2462}",
        4 => "\u{2463}",
        5 => "\u{2464}",
        6 => "\u{2465}",
        7 => "\u{2466}",
        8 => "\u{2467}",
        9 => "\u{2468}",
        10 => "\u{2469}",
        _ => "\u{2460}",
    }
}

/// Render the full advisor output.
///
/// v8.5: Grouped, action-first, decision-centric output.
/// Every recommendation is justified with explainable reasoning.
pub fn render_advisor(advisor: &CleanupAdvisor, _root: &Path) -> String {
    if advisor.opportunities.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // ── Header ──
    out.push_str(&color::section_header("PROJECT CLEANUP ADVISOR"));

    out.push_str(&format!("  {:<22} {}\n", "Project:", advisor.project_name));
    if let Some(eco) = advisor.ecosystem {
        out.push_str(&format!("  {:<22} {}\n", "Ecosystem:", eco.display()));
    }
    out.push('\n');

    // ── Opportunity Summary (P5: expanded) ──
    out.push_str(&color::section_header("SUMMARY"));

    let safe_count = advisor
        .opportunities
        .iter()
        .filter(|o| o.safe_to_clean)
        .count();

    let reclaim_pct = if advisor.directory_size > 0 {
        advisor.total_reclaimable as f64 / advisor.directory_size as f64 * 100.0
    } else {
        0.0
    };

    out.push_str(&format!(
        "  {:<22} {}\n",
        "Project size:",
        human_size(advisor.directory_size)
    ));
    out.push_str(&format!(
        "  {:<22} {} ({:.0}% of project)\n",
        "Estimated reclaim:",
        human_size(advisor.total_reclaimable),
        reclaim_pct
    ));

    // Largest reclaimable artifact
    if let Some(largest) = advisor.opportunities.iter().max_by_key(|o| o.size_bytes) {
        out.push_str(&format!(
            "  {:<22} {}\n",
            "Largest artifact:", largest.display_name
        ));
    }

    // Highest priority group
    if let Some(top_group) = advisor.groups.first() {
        out.push_str(&format!(
            "  {:<22} {} ({})\n",
            "Highest priority:",
            top_group.label,
            top_group.priority_level.display()
        ));
    }

    // Overall recommendation
    if let Some(top_group) = advisor.groups.first() {
        out.push_str(&format!(
            "  {:<22} {}\n",
            "Recommended action:", top_group.action
        ));
    }

    // Expected rebuild impact
    if let Some(slowest) = advisor
        .groups
        .iter()
        .max_by_key(|g| g.execution.regeneration_time.len())
    {
        out.push_str(&format!(
            "  {:<22} {}\n",
            "Rebuild impact:", slowest.execution.regeneration_time
        ));
    }

    out.push_str(&format!(
        "  {:<22} {} safe operation{}\n",
        "Safe operations:",
        safe_count,
        if safe_count != 1 { "s" } else { "" }
    ));

    out.push('\n');

    // ── Recommendation Cards (P1, P2: grouped, action-first) ──
    out.push_str(&color::section_header("RECOMMENDATIONS"));

    for (i, group) in advisor.groups.iter().enumerate() {
        out.push('\n');

        // Value card header: priority + label
        let label = circled_number(i + 1);
        out.push_str(&format!(
            "  {} {} {}\n",
            label,
            star_rating(group.priority.stars()),
            group.label
        ));

        // Action (primary — most important line)
        out.push_str(&format!("     Action:     {}\n", group.action));

        // Size
        out.push_str(&format!(
            "     Reclaim:    {}\n",
            human_size(group.total_size)
        ));

        // Execution cost (P6: cleanup time)
        out.push_str(&format!(
            "     Cleanup:    {}\n",
            group.execution.cleanup_time
        ));

        // Regeneration time
        out.push_str(&format!(
            "     Rebuild:    {}\n",
            group.execution.regeneration_time
        ));

        // Risk: always None for safe items (planner guarantees this)
        out.push_str(&format!("     Risk:       {}\n", color::purple("None")));

        // Confidence
        out.push_str(&format!("     Confidence: {}%\n", group.confidence_pct));

        // Items in group
        if group.items.len() > 1 {
            out.push_str("     Includes:   ");
            out.push_str(&group.items.join(", "));
            out.push('\n');
        }

        // Why this group (P9: explainability)
        if !group.reasons.is_empty() {
            out.push_str("     Why:        ");
            out.push_str(&group.reasons[0]);
            out.push('\n');
        }
    }

    out.push('\n');

    // ── Why This Order? (P4: explain ranking) ──
    if !advisor.groups.is_empty() {
        out.push_str(&color::section_header("WHY THIS ORDER?"));

        for (i, group) in advisor.groups.iter().enumerate() {
            out.push('\n');
            let label = circled_number(i + 1);
            out.push_str(&format!(
                "  {} {} — {}\n",
                label,
                group.label,
                group.priority_level.display()
            ));
            for reason in &group.ranking_reasons {
                out.push_str(&format!("  \u{2713} {reason}\n"));
            }
        }

        out.push('\n');
    }

    // ── Recommended Cleanup Order ──
    out.push_str(&color::section_header("EXECUTION PLAN"));

    for (i, group) in advisor.groups.iter().enumerate() {
        let label = circled_number(i + 1);
        out.push_str(&format!("  {} {}\n", label, group.action));
        out.push_str(&format!(
            "     {}  {}  rebuild: {}\n",
            human_size(group.total_size),
            group.priority_level.display(),
            group.execution.regeneration_time
        ));
    }

    out.push('\n');
    out.push_str("  Source code is NEVER recommended for cleanup.\n");

    out
}

// ═══════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_rust_project_with_artifacts() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"advisor-test\"\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        // Create target/ with enough content to pass 1 MB threshold
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/binary"), vec![0u8; 2_100_000]).unwrap();

        // Create .cache/ with enough content
        fs::create_dir(root.join(".cache")).unwrap();
        fs::write(root.join(".cache/data"), vec![0u8; 1_100_000]).unwrap();

        (dir, root)
    }

    fn setup_node_project_with_artifacts() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("package.json"),
            "{\"name\": \"advisor-node\", \"version\": \"1.0.0\"}",
        )
        .unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::create_dir(root.join("node_modules/pkg")).unwrap();
        // 2 MB to pass threshold
        fs::write(root.join("node_modules/pkg/index.js"), vec![0u8; 2_100_000]).unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("dist/bundle.js"), vec![0u8; 1_100_000]).unwrap();

        (dir, root)
    }

    // ── Discovery tests ──

    #[test]
    fn test_advise_rust_project_finds_target() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        assert!(
            adv.opportunities
                .iter()
                .any(|o| o.display_name == "target/"),
            "should find target/"
        );
    }

    #[test]
    fn test_advise_node_project_finds_node_modules() {
        let (_dir, root) = setup_node_project_with_artifacts();
        let adv = advise(&root);
        assert!(
            adv.opportunities
                .iter()
                .any(|o| o.display_name == "node_modules/"),
            "should find node_modules/"
        );
    }

    #[test]
    fn test_advise_skips_unsafe() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        // Source code (src/) is never recommended
        assert!(
            !adv.opportunities.iter().any(|o| o.display_name == "src/"),
            "should not recommend src/"
        );
    }

    #[test]
    fn test_advise_skips_small() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"small-test\"\n",
        )
        .unwrap();
        fs::create_dir(root.join("target")).unwrap();
        // Only 100 bytes — below MINIMUM_MEANINGFUL_SIZE
        fs::write(root.join("target/tiny"), vec![0u8; 100]).unwrap();

        let adv = advise(&root);
        assert!(
            adv.opportunities.is_empty()
                || !adv
                    .opportunities
                    .iter()
                    .any(|o| o.display_name == "target/"),
            "should skip target/ when below 1 MB"
        );
    }

    #[test]
    fn test_advise_dedup_parent_child() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        // If target/ is present, target/debug should not be listed separately
        let has_target = adv
            .opportunities
            .iter()
            .any(|o| o.display_name == "target/");
        let has_target_debug = adv
            .opportunities
            .iter()
            .any(|o| o.display_name.contains("target/debug"));
        if has_target {
            assert!(
                !has_target_debug,
                "target/debug should be deduped by target/"
            );
        }
    }

    #[test]
    fn test_advise_scores_size_heavier() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        // target/ (2 MB) should rank higher than .cache/ (1 MB)
        let target_idx = adv
            .opportunities
            .iter()
            .position(|o| o.display_name == "target/");
        let cache_idx = adv
            .opportunities
            .iter()
            .position(|o| o.display_name == ".cache/");
        if let (Some(t), Some(c)) = (target_idx, cache_idx) {
            assert!(t < c, "target/ should rank before .cache/");
        }
    }

    #[test]
    fn test_advise_ecosystem_override() {
        let (_dir, root) = setup_node_project_with_artifacts();
        let adv = advise(&root);
        let node_modules = adv
            .opportunities
            .iter()
            .find(|o| o.display_name == "node_modules/");
        if let Some(nm) = node_modules {
            assert!(
                nm.action.contains("install"),
                "node_modules action should contain 'install', got: {}",
                nm.action
            );
        }
    }

    // ── Render tests ──

    #[test]
    fn test_render_advisor() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);

        assert!(output.contains("PROJECT CLEANUP ADVISOR"));
        assert!(output.contains("SUMMARY"));
        assert!(output.contains("RECOMMENDATIONS"));
        assert!(output.contains("Safe operations:"));
        assert!(output.contains("Estimated reclaim:"));
        assert!(output.contains("of project"));
        assert!(output.contains("Highest priority:"));
    }

    #[test]
    fn test_render_advisor_has_why_ranked_1() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);
        assert!(output.contains("WHY THIS ORDER?"));
    }

    #[test]
    fn test_render_advisor_node_project() {
        let (_dir, root) = setup_node_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);

        assert!(output.contains("node_modules"));
        assert!(output.contains("install"));
    }

    #[test]
    fn test_render_advisor_has_safety_confirmation() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);
        // v8.5: Safety is implicit (all shown are safe), but
        // the footer still confirms source code is never recommended
        assert!(output.contains("Source code is NEVER recommended"));
    }

    // ── Unit tests ──

    #[test]
    fn test_size_score() {
        assert_eq!(size_score(0), 0);
        assert_eq!(size_score(500_000), 0); // < 1 MB
        assert!(size_score(2_000_000) > 0); // >= 1 MB
        assert!(size_score(200_000_000) > size_score(2_000_000)); // 200 MB > 2 MB
        assert!(size_score(6_000_000_000) >= 40); // >= 5 GB → max
    }

    #[test]
    fn test_estimate_regen_time() {
        assert_eq!(estimate_regen_time(None, 0), "Instant");
        assert_eq!(estimate_regen_time(None, 500_000), "Instant");
        // Larger sizes should return non-instant
        let large = estimate_regen_time(Some(Ecosystem::Rust), 2_000_000_000);
        assert_ne!(large, "Instant");
    }

    #[test]
    fn test_estimate_cleanup_time() {
        assert_eq!(estimate_cleanup_time("cargo clean", 0), "Instant");
        assert_eq!(estimate_cleanup_time("cargo clean", 100_000_000), "Instant");
        assert_eq!(
            estimate_cleanup_time("cargo clean", 2_000_000_000),
            "2-5 seconds"
        );
        // Non-ecosystem commands scale with size
        assert_eq!(estimate_cleanup_time("Manual cleanup", 0), "Instant");
        assert_eq!(
            estimate_cleanup_time("Manual cleanup", 200_000_000),
            "1-3 seconds"
        );
        assert_eq!(
            estimate_cleanup_time("Manual cleanup", 200_000_000_000),
            "5-15 seconds"
        );
    }

    #[test]
    fn test_ecosystem_action_override_node_modules() {
        let (action, overridden) = ecosystem_action_override(
            "node_modules",
            Some(Ecosystem::Node),
            None,
            "Remove temporary files.",
        );
        assert_eq!(action, "npm install");
        assert!(overridden);
    }

    #[test]
    fn test_ecosystem_action_override_generic_passthrough() {
        let (action, overridden) =
            ecosystem_action_override("some_dir", Some(Ecosystem::Node), None, "npm install");
        assert_eq!(action, "npm install");
        assert!(!overridden);
    }

    #[test]
    fn test_circled_number() {
        assert_eq!(circled_number(1), "\u{2460}");
        assert_eq!(circled_number(2), "\u{2461}");
        assert_eq!(circled_number(5), "\u{2464}");
    }

    #[test]
    fn test_star_rating() {
        assert_eq!(star_rating(5), "\u{2605}\u{2605}\u{2605}\u{2605}\u{2605}");
        assert_eq!(star_rating(3), "\u{2605}\u{2605}\u{2605}\u{2606}\u{2606}");
        assert_eq!(star_rating(0), "\u{2606}\u{2606}\u{2606}\u{2606}\u{2606}");
    }

    // ── v8.5 Grouping tests ──

    #[test]
    fn test_group_opportunities_same_action() {
        let opps = vec![
            CleanupOpportunity {
                display_name: "target/".to_string(),
                path: PathBuf::from("/tmp/test/target"),
                size_bytes: 700_000_000,
                safe_to_clean: true,
                action: "cargo clean".to_string(),
                reason: "Build artifacts.".to_string(),
                priority: PriorityBreakdown {
                    size_points: 33,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 12,
                    total: 90,
                },
                estimated_regen_time: "2-5 min".to_string(),
                rank: 1,
                evidence_files: vec!["Cargo.toml".to_string()],
            },
            CleanupOpportunity {
                display_name: "criterion/".to_string(),
                path: PathBuf::from("/tmp/test/criterion"),
                size_bytes: 50_000_000,
                safe_to_clean: true,
                action: "cargo clean".to_string(),
                reason: "Build artifacts.".to_string(),
                priority: PriorityBreakdown {
                    size_points: 17,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 12,
                    total: 74,
                },
                estimated_regen_time: "30-60 sec".to_string(),
                rank: 2,
                evidence_files: vec!["Cargo.toml".to_string()],
            },
        ];

        let groups = group_opportunities(&opps, Some(Ecosystem::Rust));
        assert_eq!(groups.len(), 1, "same action should produce one group");
        assert_eq!(groups[0].label, "Rust Build Artifacts");
        assert_eq!(groups[0].action, "cargo clean");
        assert_eq!(groups[0].total_size, 750_000_000);
        assert_eq!(groups[0].items.len(), 2);
        assert_eq!(groups[0].priority.total, 90); // Best score
        assert_eq!(groups[0].priority_level, PriorityLevel::Immediate);
    }

    #[test]
    fn test_group_opportunities_different_actions() {
        let opps = vec![
            CleanupOpportunity {
                display_name: "node_modules/".to_string(),
                path: PathBuf::from("/tmp/test/node_modules"),
                size_bytes: 340_000_000,
                safe_to_clean: true,
                action: "npm install".to_string(),
                reason: "Packages are re-downloadable.".to_string(),
                priority: PriorityBreakdown {
                    size_points: 28,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 12,
                    total: 85,
                },
                estimated_regen_time: "1-3 min".to_string(),
                rank: 1,
                evidence_files: vec!["package.json".to_string()],
            },
            CleanupOpportunity {
                display_name: "dist/".to_string(),
                path: PathBuf::from("/tmp/test/dist"),
                size_bytes: 95_000_000,
                safe_to_clean: true,
                action: "npm run build".to_string(),
                reason: "Build output is regenerable.".to_string(),
                priority: PriorityBreakdown {
                    size_points: 22,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 12,
                    total: 79,
                },
                estimated_regen_time: "20-60 sec".to_string(),
                rank: 2,
                evidence_files: vec!["package.json".to_string()],
            },
        ];

        let groups = group_opportunities(&opps, Some(Ecosystem::Node));
        assert_eq!(
            groups.len(),
            2,
            "different actions should produce separate groups"
        );
        assert_eq!(groups[0].label, "Node Dependencies");
        assert_eq!(groups[1].label, "Build Output");
    }

    #[test]
    fn test_group_opportunities_ordering() {
        let opps = vec![
            CleanupOpportunity {
                display_name: "dist/".to_string(),
                path: PathBuf::from("/tmp/test/dist"),
                size_bytes: 95_000_000,
                safe_to_clean: true,
                action: "npm run build".to_string(),
                reason: "Build output.".to_string(),
                priority: PriorityBreakdown {
                    size_points: 22,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 12,
                    total: 79,
                },
                estimated_regen_time: "20-60 sec".to_string(),
                rank: 1,
                evidence_files: vec![],
            },
            CleanupOpportunity {
                display_name: "node_modules/".to_string(),
                path: PathBuf::from("/tmp/test/node_modules"),
                size_bytes: 340_000_000,
                safe_to_clean: true,
                action: "npm install".to_string(),
                reason: "Packages re-downloadable.".to_string(),
                priority: PriorityBreakdown {
                    size_points: 28,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 12,
                    total: 85,
                },
                estimated_regen_time: "1-3 min".to_string(),
                rank: 2,
                evidence_files: vec![],
            },
        ];

        let groups = group_opportunities(&opps, Some(Ecosystem::Node));
        assert_eq!(
            groups[0].action, "npm install",
            "higher priority group should be first"
        );
        assert_eq!(groups[1].action, "npm run build");
    }

    #[test]
    fn test_priority_level_from_score() {
        assert_eq!(PriorityLevel::from_score(95), PriorityLevel::Immediate);
        assert_eq!(PriorityLevel::from_score(80), PriorityLevel::Immediate);
        assert_eq!(PriorityLevel::from_score(60), PriorityLevel::High);
        assert_eq!(PriorityLevel::from_score(40), PriorityLevel::Medium);
        assert_eq!(PriorityLevel::from_score(20), PriorityLevel::Low);
        assert_eq!(PriorityLevel::from_score(0), PriorityLevel::Low);
    }

    #[test]
    fn test_priority_level_ordering() {
        // PriorityLevel ordering: Lower ordinal = higher priority
        assert!(PriorityLevel::Low > PriorityLevel::Medium);
        assert!(PriorityLevel::Medium > PriorityLevel::High);
        assert!(PriorityLevel::High > PriorityLevel::Immediate);
    }

    #[test]
    fn test_derive_group_label() {
        assert_eq!(
            derive_group_label("cargo clean", Some(Ecosystem::Rust)),
            "Rust Build Artifacts"
        );
        assert_eq!(
            derive_group_label("npm install", Some(Ecosystem::Node)),
            "Node Dependencies"
        );
        assert_eq!(
            derive_group_label("pnpm install", Some(Ecosystem::Node)),
            "Node Dependencies"
        );
        assert_eq!(
            derive_group_label("npm run build", Some(Ecosystem::Node)),
            "Build Output"
        );
        assert_eq!(
            derive_group_label("next build", Some(Ecosystem::Node)),
            "Build Output"
        );
        assert_eq!(
            derive_group_label("go clean -cache", Some(Ecosystem::Go)),
            "Go Build Cache"
        );
    }

    #[test]
    fn test_ranking_reasons_content() {
        let group = CleanupGroup {
            label: "Rust Build Artifacts".to_string(),
            action: "cargo clean".to_string(),
            items: vec!["target/".to_string(), "criterion/".to_string()],
            total_size: 762_000_000,
            priority: PriorityBreakdown {
                size_points: 33,
                regenerable_points: 25,
                ecosystem_points: 20,
                confidence_points: 12,
                total: 90,
            },
            priority_level: PriorityLevel::Immediate,
            execution: ExecutionCost {
                cleanup_time: "Instant".to_string(),
                regeneration_time: "2-5 min".to_string(),
            },
            reasons: vec!["Fully regenerable from source.".to_string()],
            confidence_pct: 80,
            ranking_reasons: Vec::new(),
        };
        let reasons = ranking_reasons(&group);
        assert!(!reasons.is_empty());
        // Should mention size
        assert!(reasons.iter().any(|r| r.contains("Reclaims")));
        // Should mention size category (762 MB is < 1 GB)
        assert!(reasons.iter().any(|r| r.contains("recoverable space")));
        // Should mention regenerability
        assert!(reasons.iter().any(|r| r.contains("regenerable")));
        // Should mention ecosystem command
        assert!(reasons.iter().any(|r| r.contains("ecosystem")));
        // Should mention multiple artifacts
        assert!(reasons.iter().any(|r| r.contains("2 related")));
        // Should mention instant cleanup
        assert!(reasons.iter().any(|r| r.contains("Instant")));
    }

    #[test]
    fn test_render_advisor_has_execution_plan() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);
        assert!(output.contains("EXECUTION PLAN"));
        assert!(output.contains("Rebuild impact:"));
        assert!(output.contains("Recommended action:"));
        assert!(output.contains("Safe operations:"));
    }

    #[test]
    fn test_render_advisor_has_value_cards() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);
        // v8.5 value card fields
        assert!(output.contains("Action:"));
        assert!(output.contains("Reclaim:"));
        assert!(output.contains("Cleanup:"));
        assert!(output.contains("Rebuild:"));
        assert!(output.contains("Risk:"));
        assert!(output.contains("Confidence:"));
        assert!(output.contains("Why:"));
    }

    #[test]
    fn test_render_advisor_grouped_output() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);
        // Should have group labels, not just file names
        assert!(output.contains("Rust Build Artifacts"));
    }

    #[test]
    fn test_advisor_consistency_with_planner() {
        // P8: Advisor must never contradict planner.
        // If planner says safe, advisor shows it.
        // If planner says unsafe, advisor must not show it.
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);

        for opp in &adv.opportunities {
            // Every opportunity shown by advisor must be safe per planner
            let plan = planner::plan(&opp.path);
            assert!(
                plan.safe_to_clean,
                "Advisor shows {} but planner says unsafe",
                opp.display_name
            );
        }
    }

    #[test]
    fn test_advise_populates_groups() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        // v8.5: groups must be populated when opportunities exist
        if !adv.opportunities.is_empty() {
            assert!(!adv.groups.is_empty(), "groups should be populated");
            // Total size in groups should match total_reclaimable
            let group_total: u64 = adv.groups.iter().map(|g| g.total_size).sum();
            assert_eq!(
                group_total, adv.total_reclaimable,
                "group total should match advisor total"
            );
        }
    }
}
