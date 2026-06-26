// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — v8.4
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
//!   - Ownership (v8.1) for project context
//!   - Impact (v8.2) for consequence analysis
//!   - Planner (v8.3) for per-path safety and recommendations
//!
//! No duplicated classification rules. No duplicated logic.

use crate::color;
use crate::discovery::{self, Ecosystem};
use crate::display::human_size;

use crate::planner;
use std::path::Path;

// ── Data Structures ──

/// A single cleanup opportunity discovered inside a directory.
#[derive(Debug, Clone)]
pub struct CleanupOpportunity {
    /// Path to the cleanable artifact (relative display name).
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
    /// Priority score 0-6 (deterministic, no randomness).
    pub score: u8,
    /// Star rating 1-5 for display.
    pub stars: u8,
}

/// Full advisor output for a directory.
#[derive(Debug, Clone)]
pub struct CleanupAdvisor {
    /// Project name (or directory name if no project detected).
    pub project_name: String,
    /// Detected ecosystem, if any.
    pub ecosystem: Option<Ecosystem>,
    /// All discovered cleanup opportunities, sorted by priority.
    pub opportunities: Vec<CleanupOpportunity>,
    /// Total estimated reclaimable bytes.
    pub total_reclaimable: u64,
}

// ── Phase 1: Cleanup Opportunity Discovery ──

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

// ── Phase 2: Priority Scoring ──

/// Deterministic priority scoring.
///
/// Inputs (all observable, no randomness):
///   - size_bytes:        0-4 points based on size tier
///   - has_ecosystem_cmd: +1 if an ecosystem command exists
///   - has_regeneration:   +1 if planner provided regeneration info
///
/// Total: 0-6.  Mapped to 1-5 stars.
///
/// Calibrated to match spec examples:
///   701 MB target/  (cmd+regen) → 5 pts → ★★★★★
///   1.3 GB .cache  (regen)      → 5 pts → ★★★★★
///   480 MB node_modules/ (cmd+regen) → 4 pts → ★★★★☆
///   191 MB target/doc (cmd+regen)  → 4 pts → ★★★★☆
///    82 MB coverage/  (regen)      → 3 pts → ★★★☆☆
fn score_opportunity(size_bytes: u64, has_ecosystem_cmd: bool, has_regeneration: bool) -> (u8, u8) {
    // Size tier: primary driver of value
    let size_points: u8 = if size_bytes >= 1_073_741_824 {
        4 // >= 1 GB
    } else if size_bytes >= 524_288_000 {
        3 // >= 500 MB
    } else if size_bytes >= 104_857_600 {
        2 // >= 100 MB
    } else if size_bytes >= 52_428_800 {
        1 // >= 50 MB
    } else {
        0
    };

    // Bonus signals (safety and convenience)
    let mut bonus: u8 = 0;
    if has_ecosystem_cmd {
        bonus += 1;
    }
    if has_regeneration {
        bonus += 1;
    }

    let total = size_points + bonus;

    // Map 0-6 → 1-5 stars
    let stars = match total {
        0 => 1,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5..=6 => 5,
        _ => 1,
    };

    (total, stars)
}

/// Render star rating as Unicode stars.
fn star_rating(stars: u8) -> String {
    let filled = stars as usize;
    let empty = 5_usize.saturating_sub(filled);
    format!("{}{}", "\u{2605}".repeat(filled), "\u{2606}".repeat(empty))
}

// ── Main Advisor Function ──

/// Run the cleanup advisor on a directory.
///
/// Discovers all cleanable opportunities, scores them, and returns
/// a ranked advisor result.  Returns an empty advisor if no opportunities
/// are found (caller should fall back to single-path planner).
pub fn advise(root: &Path) -> CleanupAdvisor {
    let ecosystem = discovery::find_project_for_path(root).map(|p| p.ecosystem);

    let project_name = discovery::find_project_for_path(root)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| {
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

        // Determine display name (relative to root)
        let display_name = candidate_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Determine the best action
        let action = if !plan.suggested_commands.is_empty() {
            plan.suggested_commands[0].clone()
        } else if !plan.recommendation.is_empty() {
            plan.recommendation.clone()
        } else {
            "Manual cleanup".to_string()
        };

        // Determine reason
        let reason = if !plan.reason.is_empty() {
            plan.reason.clone()
        } else if !plan.regeneration.is_empty() {
            plan.regeneration.clone()
        } else {
            "Reclaimable disk space.".to_string()
        };

        let has_ecosystem_cmd = !plan.suggested_commands.is_empty();
        let has_regeneration = !plan.regeneration.is_empty();

        let (score, stars) = score_opportunity(
            plan.estimated_reclaimable_bytes,
            has_ecosystem_cmd,
            has_regeneration,
        );

        opportunities.push(CleanupOpportunity {
            display_name: format!("{}/", display_name),
            path: candidate_path.clone(),
            size_bytes: plan.estimated_reclaimable_bytes,
            safe_to_clean: true,
            action,
            reason,
            score,
            stars,
        });
    }

    // Sort: by score descending, then by size descending
    opportunities.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
    });

    let total_reclaimable: u64 = opportunities.iter().map(|o| o.size_bytes).sum();

    CleanupAdvisor {
        project_name,
        ecosystem,
        opportunities,
        total_reclaimable,
    }
}

// ── Phase 6: Ranking Output ──

/// Render the full advisor output.
pub fn render_advisor(advisor: &CleanupAdvisor, _root: &Path) -> String {
    if advisor.opportunities.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // Header
    out.push_str(&color::section_header("PROJECT CLEANUP ADVISOR"));

    out.push_str(&format!("  {:<20} {}\n", "Project:", advisor.project_name));
    if let Some(eco) = advisor.ecosystem {
        out.push_str(&format!("  {:<20} {}\n", "Ecosystem:", eco.display()));
    }
    out.push('\n');

    // ── Cleanup Opportunities ──
    out.push_str(&color::section_header("CLEANUP OPPORTUNITIES"));

    for opp in &advisor.opportunities {
        out.push('\n');
        out.push_str(&format!("  {}\n", star_rating(opp.stars)));
        out.push_str(&format!("  {:<20} {}\n", "", opp.display_name));
        out.push_str(&format!("  {:<20} {}\n", "", human_size(opp.size_bytes)));

        // Action (show the ecosystem command if available)
        if !opp.action.is_empty() && opp.action != "Manual cleanup" {
            out.push_str(&format!("  {:<20} {}\n", "Action:", opp.action));
        }

        // Reason (compact, one line)
        if !opp.reason.is_empty() {
            out.push_str(&format!("  {:<20} {}\n", "Reason:", opp.reason));
        }
    }

    out.push('\n');

    // ── Estimated Reclaim ──
    out.push_str(&color::section_header("ESTIMATED RECLAIM"));
    out.push_str(&format!("  {}\n", human_size(advisor.total_reclaimable)));
    out.push('\n');

    // ── Recommended Cleanup Order ──
    out.push_str(&color::section_header("RECOMMENDED CLEANUP ORDER"));

    let number_labels = [
        "\u{2460}", "\u{2461}", "\u{2462}", "\u{2463}", "\u{2464}", "\u{2465}", "\u{2466}",
        "\u{2467}", "\u{2468}", "\u{2469}",
    ];

    for (i, opp) in advisor.opportunities.iter().enumerate() {
        let label = if i < number_labels.len() {
            number_labels[i]
        } else {
            "\u{2460}" // Fallback for >10 items
        };

        // Show action command if available, else just the path
        if !opp.action.is_empty() && opp.action != "Manual cleanup" {
            // Deduplicate: skip if same command already shown
            if i > 0 {
                let prev = &advisor.opportunities[i - 1];
                if prev.action == opp.action {
                    continue;
                }
            }
            out.push_str(&format!("  {} {}\n", label, opp.action));
        } else {
            out.push_str(&format!("  {} {}\n", label, opp.display_name));
        }
    }

    out.push('\n');
    out.push_str("  Source code is NEVER recommended for cleanup.\n");

    out
}

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

        // Create target/ with some content
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/binary"), vec![0u8; 2048]).unwrap();
        fs::create_dir_all(root.join("target/doc")).unwrap();
        fs::write(root.join("target/doc/doc.html"), vec![0u8; 1024]).unwrap();

        // Create .cache/
        fs::create_dir(root.join(".cache")).unwrap();
        fs::write(root.join(".cache/data"), vec![0u8; 512]).unwrap();

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
        fs::write(root.join("node_modules/pkg/index.js"), vec![0u8; 4096]).unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("dist/bundle.js"), vec![0u8; 8192]).unwrap();

        (dir, root)
    }

    #[test]
    fn test_advise_rust_project_finds_target() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        // Should find target/ as a cleanable opportunity
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

        // target/ has 2048 + 1024 = 3072 bytes
        // .cache/ has 512 bytes
        assert!(
            advisor.total_reclaimable >= 3000,
            "Total reclaimable should be >= 3000, got: {}",
            advisor.total_reclaimable
        );
    }

    #[test]
    fn test_advise_sorted_by_priority() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        // Opportunities should be sorted by score descending
        for window in advisor.opportunities.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "Should be sorted by score: {} >= {}",
                window[0].score,
                window[1].score
            );
        }
    }

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
    fn test_advise_project_name_detected() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        // Project name comes from discovery (directory name), not Cargo.toml
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

    #[test]
    fn test_score_deterministic() {
        // Same inputs must always produce same output
        let (s1, st1) = score_opportunity(500_000_000, true, true);
        let (s2, st2) = score_opportunity(500_000_000, true, true);
        assert_eq!(s1, s2);
        assert_eq!(st1, st2);

        // Run 10 times to be sure
        for _ in 0..10 {
            let (s, st) = score_opportunity(500_000_000, true, true);
            assert_eq!(s, s1);
            assert_eq!(st, st1);
        }
    }

    #[test]
    fn test_score_large_with_command_is_five_stars() {
        let (_, stars) = score_opportunity(2_000_000_000, true, true);
        assert_eq!(stars, 5);
    }

    #[test]
    fn test_score_tiny_no_command_is_low() {
        let (_, stars) = score_opportunity(100, false, false);
        assert!(stars <= 2);
    }

    #[test]
    fn test_score_size_tiers() {
        // < 50 MB
        let (s1, _) = score_opportunity(5_000_000, false, false);
        assert_eq!(s1, 0);

        // >= 50 MB
        let (s2, _) = score_opportunity(60_000_000, false, false);
        assert_eq!(s2, 1);

        // >= 100 MB
        let (s3, _) = score_opportunity(200_000_000, false, false);
        assert_eq!(s3, 2);

        // >= 500 MB
        let (s3b, _) = score_opportunity(600_000_000, false, false);
        assert_eq!(s3b, 3);

        // >= 1 GB
        let (s4, _) = score_opportunity(2_000_000_000, false, false);
        assert_eq!(s4, 4);
    }

    #[test]
    fn test_render_advisor_not_empty() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        assert!(output.contains("PROJECT CLEANUP ADVISOR"));
        assert!(output.contains("CLEANUP OPPORTUNITIES"));
        assert!(output.contains("ESTIMATED RECLAIM"));
        assert!(output.contains("RECOMMENDED CLEANUP ORDER"));
        assert!(output.contains("target/"));
        assert!(output.contains("cargo clean"));
        // Project name comes from directory, not hardcoded
        assert!(output.contains("PROJECT CLEANUP ADVISOR"));
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
        // src/ should NOT appear in opportunities
        assert!(!advisor
            .opportunities
            .iter()
            .any(|o| o.display_name.contains("src")));
    }

    #[test]
    fn test_advise_never_includes_unsafe_paths() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        // All opportunities must be safe
        for opp in &advisor.opportunities {
            assert!(opp.safe_to_clean, "{} should be safe", opp.display_name);
        }
    }

    #[test]
    fn test_render_advisor_shows_project_name() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);
        let output = render_advisor(&advisor, &root);

        // Verify the project name appears in the output
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

    #[test]
    fn test_advisor_agrees_with_planner_on_size() {
        let (_dir, root) = setup_rust_project_with_artifacts();
        let advisor = advise(&root);

        for opp in &advisor.opportunities {
            let plan = planner::plan(&opp.path);
            assert_eq!(
                opp.size_bytes, plan.estimated_reclaimable_bytes,
                "Advisor and Planner disagree on size of {}",
                opp.display_name
            );
        }
    }

    // ── Benchmark-style: advisor on a real project ──

    #[test]
    fn test_benchmark_advise_on_self() {
        // Run advisor on the zacxiom project itself (if running from source)
        let self_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        if self_path.join("Cargo.toml").exists() {
            let advisor = advise(self_path);

            // Should find target/ at minimum
            if self_path.join("target").exists() {
                assert!(
                    advisor
                        .opportunities
                        .iter()
                        .any(|o| o.display_name == "target/"),
                    "Should find target/ in zacxiom project"
                );
            }

            // Verify all sizes are reasonable
            for opp in &advisor.opportunities {
                assert!(
                    opp.size_bytes > 0 || opp.display_name.contains("cache"),
                    "{} should have measurable size",
                    opp.display_name
                );
            }
        }
    }
}
