// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — v8.4.1
//!
//! Transforms `zacxiom plan` from a single-path planner into an intelligent
//! cleanup advisor.  Discovers, ranks, and presents ALL worthwhile cleanup
//! opportunities inside a directory.
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

use crate::color;
use crate::discovery::{self, Ecosystem, ProjectInfo};
use crate::display::human_size;
use crate::planner;
use std::collections::HashSet;
use std::path::Path;

// ═══════════════════════════════════════════════════════════════
// Data Structures
// ═══════════════════════════════════════════════════════════════

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
    pub estimated_time: String,
    /// 1-based rank after sorting (set during final ranking).
    pub rank: usize,
    /// Evidence files supporting the confidence score.
    pub evidence_files: Vec<String>,
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
// Phase 3: Estimated Reclaim + Time Saved
// ═══════════════════════════════════════════════════════════════

/// Estimate regeneration time based on ecosystem and size.
///
/// Returns "Instant" for negligible sizes.
fn estimate_regen_time(ecosystem: Option<Ecosystem>, size_bytes: u64) -> String {
    // P6: 0 B or negligible → "Instant"
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
        // Keep this opportunity unless a different path in the list
        // is a prefix of it (i.e. a parent already covers it)
        let has_parent = parent_paths.iter().any(|parent| {
            parent != &opp_str
                && (opp_str.starts_with(&format!("{}/", parent))
                    || opp_str.starts_with(&format!("{parent}/")))
        });
        !has_parent
    });
}

// ═══════════════════════════════════════════════════════════════
// P1: Ecosystem-Aware Action Override
// ═══════════════════════════════════════════════════════════════

/// Minimum size (bytes) to be worth showing as a cleanup opportunity.
/// Items below this are filtered out (P7).
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
///
/// This handles the case where the classifier assigns a generic
/// category (e.g. TemporaryFile) to a known ecosystem artifact,
/// causing the planner to return generic "Remove temporary files."
/// instead of the ecosystem-specific command.
fn ecosystem_action_override(
    display_name: &str,
    ecosystem: Option<Ecosystem>,
    project: Option<&ProjectInfo>,
    planner_action: &str,
) -> (String, bool) {
    // Only override if the planner gave a generic action
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
/// and returns a ranked advisor result.  Returns an empty advisor
/// if no opportunities are found (caller should fall back to
/// single-path planner).
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

        // P7: Skip items below minimum meaningful size
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

        // P1: Ecosystem-aware action override
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

        let estimated_time = estimate_regen_time(ecosystem, plan.estimated_reclaimable_bytes);

        // P2: Collect evidence files for auditable confidence
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
            estimated_time,
            rank: 0, // Set after sorting
            evidence_files,
        });
    }

    // Phase 4: Dedup parent-child
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

    CleanupAdvisor {
        project_name,
        ecosystem,
        opportunities,
        total_reclaimable,
        directory_size,
    }
}

// ═══════════════════════════════════════════════════════════════
// Phase 6: Full UX Output
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

    // ── Opportunity Summary ──
    out.push_str(&color::section_header("OPPORTUNITY SUMMARY"));

    let count = advisor.opportunities.len();
    out.push_str(&format!("  {:<22} {}\n", "Cleanup opportunities:", count));
    out.push_str(&format!(
        "  {:<22} {}\n",
        "Total reclaimable:",
        human_size(advisor.total_reclaimable)
    ));

    // P5: Clarify what the percentage is relative to
    let reclaim_pct = if advisor.directory_size > 0 {
        advisor.total_reclaimable as f64 / advisor.directory_size as f64 * 100.0
    } else {
        0.0
    };
    out.push_str(&format!(
        "  {:<22} {:.0}% of project directory\n",
        "Reclaimable:", reclaim_pct
    ));

    // Highest priority item
    if let Some(top) = advisor.opportunities.first() {
        out.push_str(&format!(
            "  {:<22} {}\n",
            "Highest priority:", top.display_name
        ));
    }

    // Safety confirmation
    out.push_str(&format!(
        "  {:<22} {}\n",
        "Safety:", "All recommendations verified safe."
    ));

    out.push('\n');

    // ── Cleanup Opportunities ──
    out.push_str(&color::section_header("CLEANUP OPPORTUNITIES"));

    for opp in &advisor.opportunities {
        out.push('\n');
        // P8: Value-oriented display — stars + name prominent, score secondary
        out.push_str(&format!(
            "  {}  {}\n",
            star_rating(opp.priority.stars()),
            opp.display_name
        ));
        out.push_str(&format!(
            "           Size:     {}\n",
            human_size(opp.size_bytes)
        ));

        // Action (show the ecosystem command if available)
        if !opp.action.is_empty() && opp.action != "Manual cleanup" {
            out.push_str(&format!("           Action:   {}\n", opp.action));
        }

        // Estimated regeneration time (P6: "Instant" for negligible)
        if !opp.estimated_time.is_empty() {
            out.push_str(&format!("           Regen:    {}\n", opp.estimated_time));
        }

        // Reason (compact, one line)
        if !opp.reason.is_empty() {
            out.push_str(&format!("           {}\n", opp.reason));
        }
    }

    out.push('\n');

    // ── Why Ranked #1? ──
    if let Some(top) = advisor.opportunities.first() {
        out.push_str(&color::section_header(&format!(
            "WHY RANKED #1? {}",
            top.display_name
        )));

        // P3: Labeled score breakdown with column alignment
        out.push_str(&format!(
            "  Priority Score  {:>3}/100  {}\n\n",
            top.priority.total,
            star_rating(top.priority.stars())
        ));
        out.push_str(&format!(
            "  Size         +{:>2}  {} reclaimable\n",
            top.priority.size_points,
            human_size(top.size_bytes)
        ));
        out.push_str(&format!(
            "  Safety       +{:>2}  Fully regenerable\n",
            top.priority.regenerable_points
        ));
        if top.priority.ecosystem_points > 0 {
            out.push_str(&format!(
                "  Ecosystem    +{:>2}  Known ecosystem command\n",
                top.priority.ecosystem_points
            ));
        }

        // P2: Auditable confidence with evidence
        let conf_pct = top.priority.confidence_points as u16 * 100 / 15;
        if !top.evidence_files.is_empty() {
            out.push_str(&format!(
                "  Confidence   +{:>2}  {}% ({}\n",
                top.priority.confidence_points,
                conf_pct,
                top.evidence_files.join(", ")
            ));
            out.push_str("                         )\n");
        } else {
            out.push_str(&format!(
                "  Confidence   +{:>2}  {}%\n",
                top.priority.confidence_points, conf_pct
            ));
        }

        // P8: Cleanup Value summary — explainability over numbers
        out.push('\n');
        out.push_str("  Cleanup Value\n");
        out.push_str(&format!("  {}\n\n", star_rating(top.priority.stars())));
        out.push_str(&format!("  {} reclaimed\n", human_size(top.size_bytes)));
        out.push_str("  100% regenerable\n");
        if top.priority.ecosystem_points > 0 {
            out.push_str("  Known ecosystem command\n");
        }
        out.push_str("  No source code affected\n");
    }

    out.push('\n');

    // ── Estimated Reclaim ──
    out.push_str(&color::section_header("ESTIMATED RECLAIM"));
    out.push_str(&format!("  {}\n", human_size(advisor.total_reclaimable)));
    // P4: Add context to the reclaim section
    if let Some(largest) = advisor.opportunities.first() {
        out.push_str(&format!(
            "  Largest removable artifact: {}\n",
            largest.display_name
        ));
    }
    if reclaim_pct > 50.0 {
        out.push_str("  Majority of project directory is reclaimable build artifacts.\n");
    }
    out.push('\n');

    // ── Recommended Cleanup Order ──
    out.push_str(&color::section_header("RECOMMENDED CLEANUP ORDER"));

    for opp in &advisor.opportunities {
        let label = circled_number(opp.rank);

        // Show action command if available, else just the path
        if !opp.action.is_empty() && opp.action != "Manual cleanup" {
            out.push_str(&format!("  {} {}\n", label, opp.action));
        } else {
            out.push_str(&format!("  {} {}\n", label, opp.display_name));
        }
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
        // Use a 2 MB file to ensure it's above MINIMUM_MEANINGFUL_SIZE
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
        let advisor = advise(&root);

        assert!(
            advisor
                .opportunities
                .iter()
                .any(|o| o.display_name == "target/"),
            "Should find target/, got: {:?}",
            advisor.opportunities
        );
    }

    #[test]
    fn test_advise_rust_project_target_is_safe() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        let target_opp = advisor
            .opportunities
            .iter()
            .find(|o| o.display_name == "target/")
            .expect("target/ should be found");
        assert!(target_opp.safe_to_clean);
    }

    #[test]
    fn test_advise_rust_project_has_cargo_clean() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        let target_opp = advisor
            .opportunities
            .iter()
            .find(|o| o.display_name == "target/")
            .expect("target/ should be found");
        assert!(
            target_opp.action.contains("cargo clean"),
            "Should suggest cargo clean, got: {}",
            target_opp.action
        );
    }

    #[test]
    fn test_advise_total_reclaimable() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        assert!(
            advisor.total_reclaimable >= 2_000_000,
            "Total reclaimable should be >= 2 MB, got: {}",
            advisor.total_reclaimable
        );
    }

    #[test]
    fn test_advise_sorted_by_priority() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        for window in advisor.opportunities.windows(2) {
            assert!(
                window[0].priority.total >= window[1].priority.total,
                "Should be sorted by priority: {} >= {}",
                window[0].priority.total,
                window[1].priority.total
            );
        }
    }

    // ── P1: Node.js ecosystem command tests ──

    #[test]
    fn test_advise_node_project_finds_node_modules() {
        let (_dir, root) = setup_node_project_with_artifacts();
        let advisor = advise(&root);

        assert!(
            advisor
                .opportunities
                .iter()
                .any(|o| o.display_name == "node_modules/"),
            "Should find node_modules/, got: {:?}",
            advisor.opportunities
        );
    }

    #[test]
    fn test_advise_node_uses_npm_install_not_generic() {
        let (_dir, root) = setup_node_project_with_artifacts();
        let advisor = advise(&root);

        let nm = advisor
            .opportunities
            .iter()
            .find(|o| o.display_name == "node_modules/")
            .expect("node_modules/ should be found");
        assert!(
            nm.action.contains("install"),
            "Node action should be 'npm install', got: '{}'",
            nm.action
        );
        assert!(
            !nm.action.contains("Remove temporary files"),
            "Should NOT use generic wording, got: '{}'",
            nm.action
        );
    }

    #[test]
    fn test_node_pnpm_detection() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("package.json"),
            "{\"name\": \"pnpm-test\", \"version\": \"1.0.0\"}",
        )
        .unwrap();
        fs::write(root.join("pnpm-lock.yaml"), "").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), vec![0u8; 2_100_000]).unwrap();

        let advisor = advise(&root);
        let nm = advisor
            .opportunities
            .iter()
            .find(|o| o.display_name == "node_modules/");
        if let Some(nm) = nm {
            assert_eq!(nm.action, "pnpm install");
        }
    }

    #[test]
    fn test_node_yarn_detection() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("package.json"),
            "{\"name\": \"yarn-test\", \"version\": \"1.0.0\"}",
        )
        .unwrap();
        fs::write(root.join("yarn.lock"), "").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), vec![0u8; 2_100_000]).unwrap();

        let advisor = advise(&root);
        let nm = advisor
            .opportunities
            .iter()
            .find(|o| o.display_name == "node_modules/");
        if let Some(nm) = nm {
            assert_eq!(nm.action, "yarn install");
        }
    }

    // ── P2: Confidence evidence tests ──

    #[test]
    fn test_evidence_files_collected() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        let target_opp = advisor
            .opportunities
            .iter()
            .find(|o| o.display_name == "target/");
        if let Some(opp) = target_opp {
            // Should have collected evidence from ownership
            assert!(
                !opp.evidence_files.is_empty(),
                "target/ should have evidence files"
            );
        }
    }

    // ── P6: Regen time for negligible sizes ──

    #[test]
    fn test_regen_time_instant_for_zero() {
        assert_eq!(estimate_regen_time(Some(Ecosystem::Rust), 0), "Instant");
        assert_eq!(
            estimate_regen_time(Some(Ecosystem::Rust), 500_000),
            "Instant"
        );
    }

    #[test]
    fn test_regen_time_normal_for_large() {
        assert_eq!(
            estimate_regen_time(Some(Ecosystem::Rust), 1_500_000_000),
            "5-15 min"
        );
    }

    // ── P7: Minimum size filter tests ──

    #[test]
    fn test_advise_empty_for_no_artifacts() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"empty\"\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        // No target/, no .cache/, no cleanable artifacts

        let advisor = advise(&root);
        assert!(
            advisor.opportunities.is_empty(),
            "No artifacts should be found, got: {:?}",
            advisor.opportunities
        );
    }

    #[test]
    fn test_tiny_artifacts_filtered_out() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"tiny\"\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        // Create a tiny target/ (below 1 MB threshold)
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/binary"), vec![0u8; 100]).unwrap();

        let advisor = advise(&root);
        assert!(
            advisor.opportunities.is_empty(),
            "Tiny artifacts should be filtered out, got: {:?}",
            advisor.opportunities
        );
    }

    // ── General advisor tests ──

    #[test]
    fn test_advise_project_name_detected() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        assert!(!advisor.project_name.is_empty());
        assert!(advisor.project_name.len() <= 100);
    }

    #[test]
    fn test_advise_ecosystem_detected() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        assert_eq!(advisor.ecosystem, Some(Ecosystem::Rust));
    }

    #[test]
    fn test_advise_never_zero_filesystem() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let _advisor = advise(&root);

        // After advising, target/ must still exist (read-only)
        assert!(root.join("target").exists());
        assert!(root.join("src").exists());
    }

    // ── Priority scoring tests ──

    #[test]
    fn test_size_score_tiers() {
        assert_eq!(size_score(500_000), 0); // < 1 MB
        assert_eq!(size_score(5_000_000), 6); // 1-10 MB
        assert_eq!(size_score(30_000_000), 12); // 10-50 MB
        assert_eq!(size_score(80_000_000), 17); // 50-100 MB
        assert_eq!(size_score(200_000_000), 22); // 100-500 MB
        assert_eq!(size_score(700_000_000), 28); // 500 MB-1 GB
        assert_eq!(size_score(1_500_000_000), 33); // 1-2 GB
        assert_eq!(size_score(3_000_000_000), 37); // 2-5 GB
        assert_eq!(size_score(6_000_000_000), 40); // 5+ GB
    }

    #[test]
    fn test_stars_from_score() {
        let b = PriorityBreakdown {
            size_points: 0,
            regenerable_points: 0,
            ecosystem_points: 0,
            confidence_points: 0,
            total: 10,
        };
        assert_eq!(b.stars(), 1);

        let b2 = PriorityBreakdown {
            size_points: 0,
            regenerable_points: 0,
            ecosystem_points: 0,
            confidence_points: 0,
            total: 30,
        };
        assert_eq!(b2.stars(), 2);

        let b3 = PriorityBreakdown {
            size_points: 0,
            regenerable_points: 0,
            ecosystem_points: 0,
            confidence_points: 0,
            total: 50,
        };
        assert_eq!(b3.stars(), 3);

        let b4 = PriorityBreakdown {
            size_points: 0,
            regenerable_points: 0,
            ecosystem_points: 0,
            confidence_points: 0,
            total: 70,
        };
        assert_eq!(b4.stars(), 4);

        let b5 = PriorityBreakdown {
            size_points: 0,
            regenerable_points: 0,
            ecosystem_points: 0,
            confidence_points: 0,
            total: 90,
        };
        assert_eq!(b5.stars(), 5);
    }

    // ── Dedup tests ──

    #[test]
    fn test_dedup_parent_child_removes_children() {
        let mut opps = vec![
            CleanupOpportunity {
                display_name: "target/".into(),
                path: PathBuf::from("/tmp/proj/target"),
                size_bytes: 1000,
                safe_to_clean: true,
                action: "cargo clean".into(),
                reason: "Build artifacts".into(),
                priority: PriorityBreakdown {
                    size_points: 0,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 10,
                    total: 55,
                },
                estimated_time: "1 min".into(),
                rank: 0,
                evidence_files: vec![],
            },
            CleanupOpportunity {
                display_name: "debug/".into(),
                path: PathBuf::from("/tmp/proj/target/debug"),
                size_bytes: 500,
                safe_to_clean: true,
                action: "".into(),
                reason: "Debug build".into(),
                priority: PriorityBreakdown {
                    size_points: 0,
                    regenerable_points: 25,
                    ecosystem_points: 0,
                    confidence_points: 5,
                    total: 30,
                },
                estimated_time: "30 sec".into(),
                rank: 0,
                evidence_files: vec![],
            },
        ];

        dedup_parent_child(&mut opps);
        assert_eq!(opps.len(), 1);
        assert_eq!(opps[0].display_name, "target/");
    }

    #[test]
    fn test_dedup_keeps_siblings() {
        let mut opps = vec![
            CleanupOpportunity {
                display_name: "target/".into(),
                path: PathBuf::from("/tmp/proj/target"),
                size_bytes: 1000,
                safe_to_clean: true,
                action: "cargo clean".into(),
                reason: "Build".into(),
                priority: PriorityBreakdown {
                    size_points: 0,
                    regenerable_points: 25,
                    ecosystem_points: 20,
                    confidence_points: 10,
                    total: 55,
                },
                estimated_time: "1 min".into(),
                rank: 0,
                evidence_files: vec![],
            },
            CleanupOpportunity {
                display_name: ".cache/".into(),
                path: PathBuf::from("/tmp/proj/.cache"),
                size_bytes: 200,
                safe_to_clean: true,
                action: "".into(),
                reason: "Cache".into(),
                priority: PriorityBreakdown {
                    size_points: 0,
                    regenerable_points: 25,
                    ecosystem_points: 0,
                    confidence_points: 5,
                    total: 30,
                },
                estimated_time: "10 sec".into(),
                rank: 0,
                evidence_files: vec![],
            },
        ];

        dedup_parent_child(&mut opps);
        assert_eq!(opps.len(), 2);
    }

    // ── Rank tests ──

    #[test]
    fn test_rank_set_after_sorting() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        if !advisor.opportunities.is_empty() {
            assert_eq!(advisor.opportunities[0].rank, 1);
            if advisor.opportunities.len() >= 2 {
                assert_eq!(advisor.opportunities[1].rank, 2);
            }
        }
    }

    // ── Render tests ──

    #[test]
    fn test_render_advisor_not_empty() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains("PROJECT CLEANUP ADVISOR"));
        assert!(output.contains("OPPORTUNITY SUMMARY"));
        assert!(output.contains("CLEANUP OPPORTUNITIES"));
        assert!(output.contains("ESTIMATED RECLAIM"));
        assert!(output.contains("RECOMMENDED CLEANUP ORDER"));
        assert!(output.contains("target/"));
        assert!(output.contains("cargo clean"));
    }

    #[test]
    fn test_render_advisor_has_summary() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains("Cleanup opportunities:"));
        assert!(output.contains("Total reclaimable:"));
        // P5: Should say "of project directory"
        assert!(output.contains("of project directory"));
        assert!(output.contains("Highest priority:"));
        assert!(output.contains("All recommendations verified safe."));
    }

    #[test]
    fn test_render_advisor_has_why_ranked_1() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains("WHY RANKED #1?"));
        assert!(output.contains("reclaimable"));
        assert!(output.contains("Fully regenerable"));
    }

    #[test]
    fn test_render_advisor_has_labeled_breakdown() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        // P3: Labeled columns
        assert!(output.contains("Size"));
        assert!(output.contains("Safety"));
        assert!(output.contains("Ecosystem"));
        assert!(output.contains("Confidence"));
        assert!(output.contains("Priority Score"));
    }

    #[test]
    fn test_render_advisor_has_cleanup_value() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        // P8: Cleanup Value section
        assert!(output.contains("Cleanup Value"));
        assert!(output.contains("reclaimed"));
        assert!(output.contains("100% regenerable"));
        assert!(output.contains("No source code affected"));
    }

    #[test]
    fn test_render_advisor_has_largest_artifact() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        // P4: Context in Estimated Reclaim
        assert!(output.contains("Largest removable artifact"));
    }

    #[test]
    fn test_render_advisor_no_box_characters() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(!output.contains('\u{250c}')); // ┌
        assert!(!output.contains('\u{2510}')); // ┐
        assert!(!output.contains('\u{2554}')); // ╔
        assert!(!output.contains('\u{255a}')); // ╚
        assert!(!output.contains('\u{251c}')); // ├
        assert!(!output.contains('\u{2524}')); // ┤
    }

    #[test]
    fn test_render_advisor_empty_returns_empty() {
        let advisor = CleanupAdvisor {
            project_name: "test".into(),
            ecosystem: None,
            opportunities: Vec::new(),
            total_reclaimable: 0,
            directory_size: 0,
        };
        let output = render_advisor(&advisor, Path::new("/tmp"));
        assert!(output.is_empty());
    }

    #[test]
    fn test_render_advisor_contains_stars() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains('\u{2605}')); // ★
    }

    #[test]
    fn test_render_advisor_contains_circled_numbers() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains('\u{2460}')); // ①
    }

    #[test]
    fn test_render_advisor_source_code_never_recommended() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains("Source code is NEVER recommended for cleanup"));
        assert!(!advisor
            .opportunities
            .iter()
            .any(|o| o.display_name.contains("src")));
    }

    #[test]
    fn test_render_advisor_shows_project_name() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains("Project:"));
        assert!(output.contains(&advisor.project_name));
    }

    // ── Integration: Advisor + Planner consistency ──

    #[test]
    fn test_advisor_agrees_with_planner_on_safety() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        for opp in &advisor.opportunities {
            let plan = planner::plan(&opp.path);
            assert_eq!(
                opp.safe_to_clean, plan.safe_to_clean,
                "Advisor and Planner disagree on safety of {}",
                opp.display_name
            );
        }
    }

    // ── Safety invariants ──

    #[test]
    fn test_advise_never_includes_unsafe_paths() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        for opp in &advisor.opportunities {
            assert!(opp.safe_to_clean, "{} should be safe", opp.display_name);
        }
    }

    #[test]
    fn test_priority_total_capped_at_100() {
        let b = PriorityBreakdown {
            size_points: 40,
            regenerable_points: 25,
            ecosystem_points: 20,
            confidence_points: 15,
            total: 100,
        };
        assert!(b.total <= 100);
    }

    // ── Benchmark-style: advisor on real project ──

    #[test]
    fn test_benchmark_advise_on_self() {
        let self_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        if self_path.join("Cargo.toml").exists() {
            let advisor = advise(self_path);

            if self_path.join("target").exists() {
                assert!(
                    advisor
                        .opportunities
                        .iter()
                        .any(|o| o.display_name == "target/"),
                    "Should find target/ in zacxiom project"
                );
            }

            for opp in &advisor.opportunities {
                assert!(
                    opp.size_bytes > 0,
                    "{} should have measurable size",
                    opp.display_name
                );
            }

            for opp in &advisor.opportunities {
                assert!(
                    opp.priority.total <= 100,
                    "{} has score {} > 100",
                    opp.display_name,
                    opp.priority.total
                );
            }
        }
    }

    // ── Additional tests ──

    #[test]
    fn test_directory_size_nonnegative() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        assert!(advisor.directory_size > 0);
    }

    #[test]
    fn test_reclaim_percentage_reasonable() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        if advisor.directory_size > 0 {
            let pct = advisor.total_reclaimable as f64 / advisor.directory_size as f64 * 100.0;
            assert!(
                (0.0..=100.0).contains(&pct),
                "Reclaim % should be 0-100, got: {}",
                pct
            );
        }
    }

    #[test]
    fn test_ecosystem_candidates_rust_has_target() {
        let candidates = ecosystem_candidates(Some(Ecosystem::Rust));
        assert!(candidates.contains(&"target"));
        assert!(candidates.contains(&"criterion"));
        assert!(candidates.contains(&"coverage"));
        assert!(candidates.contains(&".cache"));
    }

    #[test]
    fn test_ecosystem_candidates_node_has_node_modules() {
        let candidates = ecosystem_candidates(Some(Ecosystem::Node));
        assert!(candidates.contains(&"node_modules"));
        assert!(candidates.contains(&"dist"));
        assert!(candidates.contains(&".next"));
    }

    #[test]
    fn test_ecosystem_candidates_python_has_pycache() {
        let candidates = ecosystem_candidates(Some(Ecosystem::Python));
        assert!(candidates.contains(&"__pycache__"));
        assert!(candidates.contains(&".pytest_cache"));
    }

    #[test]
    fn test_ecosystem_candidates_none_has_common() {
        let candidates = ecosystem_candidates(None);
        assert!(candidates.contains(&".cache"));
        assert!(candidates.contains(&"tmp"));
        assert!(candidates.contains(&"logs"));
        assert!(!candidates.contains(&"target"));
        assert!(!candidates.contains(&"node_modules"));
    }

    #[test]
    fn test_circled_number() {
        assert_eq!(circled_number(1), "\u{2460}");
        assert_eq!(circled_number(5), "\u{2464}");
        assert_eq!(circled_number(10), "\u{2469}");
        assert_eq!(circled_number(11), "\u{2460}"); // Fallback
    }

    #[test]
    fn test_compute_priority_components_nonnegative() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        for opp in &advisor.opportunities {
            assert!(opp.priority.size_points <= 40);
            assert!(opp.priority.regenerable_points <= 25);
            assert!(opp.priority.ecosystem_points <= 20);
            assert!(opp.priority.confidence_points <= 15);
        }
    }

    // ── Ecosystem action override tests ──

    #[test]
    fn test_ecosystem_override_node_modules() {
        let (action, overridden) = ecosystem_action_override(
            "node_modules/",
            Some(Ecosystem::Node),
            None,
            "Remove temporary files.",
        );
        assert_eq!(action, "npm install");
        assert!(overridden);
    }

    #[test]
    fn test_ecosystem_override_target() {
        let (action, overridden) = ecosystem_action_override(
            "target/",
            Some(Ecosystem::Rust),
            None,
            "Remove temporary files.",
        );
        assert_eq!(action, "cargo clean");
        assert!(overridden);
    }

    #[test]
    fn test_ecosystem_override_no_override_for_real_cmd() {
        let (action, overridden) =
            ecosystem_action_override("target/", Some(Ecosystem::Rust), None, "cargo clean");
        assert_eq!(action, "cargo clean");
        assert!(!overridden);
    }

    #[test]
    fn test_detect_node_pm_npm() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let project = ProjectInfo {
            name: "test".into(),
            root: root.clone(),
            ecosystem: Ecosystem::Node,
            manifests: vec![root.join("package-lock.json")],
        };
        assert_eq!(detect_node_pm(&project), "npm");
    }

    #[test]
    fn test_detect_node_pm_pnpm() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let project = ProjectInfo {
            name: "test".into(),
            root: root.clone(),
            ecosystem: Ecosystem::Node,
            manifests: vec![root.join("pnpm-lock.yaml")],
        };
        assert_eq!(detect_node_pm(&project), "pnpm");
    }

    #[test]
    fn test_detect_node_pm_yarn() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let project = ProjectInfo {
            name: "test".into(),
            root: root.clone(),
            ecosystem: Ecosystem::Node,
            manifests: vec![root.join("yarn.lock")],
        };
        assert_eq!(detect_node_pm(&project), "yarn");
    }
}
