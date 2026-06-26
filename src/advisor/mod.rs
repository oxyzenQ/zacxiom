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

pub(crate) mod discover;
pub(crate) mod execution;
pub(crate) mod grouping;
pub(crate) mod render;
pub(crate) mod scoring;
pub mod types;

pub use execution::advise;
pub use render::render_advisor;

// Re-export sub-module items needed by external code and tests

#[cfg(test)]
mod tests {
    use super::discover::ecosystem_action_override;
    use super::execution::{estimate_cleanup_time, estimate_regen_time};
    use super::grouping::{derive_group_label, group_opportunities, ranking_reasons};
    use super::render::{circled_number, ecosystem_regen_label, star_rating};
    use super::scoring::size_score;
    use super::types::{
        CleanupGroup, CleanupOpportunity, ExecutionCost, PriorityBreakdown, PriorityLevel,
    };
    use super::*;
    use crate::discovery::Ecosystem;
    use crate::planner;
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

    // ── v8.5.1: UX Polish tests ──

    #[test]
    fn test_p3_risk_says_verified_safe_not_none() {
        // P3: Advisor risk must say "Verified Safe", not "None"
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        let output = render_advisor(&adv, &root);
        assert!(
            output.contains("Verified Safe"),
            "Advisor output should contain 'Verified Safe', got:\n{}",
            output
        );
        assert!(
            !output.contains("Risk:       None") && !output.contains("Risk:       \u{1b}"),
            "Advisor output should NOT contain 'Risk: None'"
        );
    }

    #[test]
    fn test_p5_rust_uses_rebuild_label() {
        // P5: Rust ecosystem should use "Rebuild" label
        let (_dir, root) = setup_rust_project_with_artifacts();
        let adv = advise(&root);
        assert_eq!(adv.ecosystem, Some(Ecosystem::Rust));
        let output = render_advisor(&adv, &root);
        assert!(
            output.contains("Rebuild:"),
            "Rust advisor should use 'Rebuild:' label, got:\n{}",
            output
        );
    }

    #[test]
    fn test_p5_node_uses_reinstall_label() {
        // P5: Node ecosystem should use "Reinstall time" label
        let (_dir, root) = setup_node_project_with_artifacts();
        let adv = advise(&root);
        assert_eq!(adv.ecosystem, Some(Ecosystem::Node));
        let output = render_advisor(&adv, &root);
        assert!(
            output.contains("Reinstall time:"),
            "Node advisor should use 'Reinstall time:' label, got:\n{}",
            output
        );
    }

    #[test]
    fn test_p5_ecosystem_regen_label_helper() {
        // P5: Unit test for the helper function
        assert_eq!(ecosystem_regen_label(Some(Ecosystem::Rust)), "Rebuild");
        assert_eq!(
            ecosystem_regen_label(Some(Ecosystem::Node)),
            "Reinstall time"
        );
        assert_eq!(
            ecosystem_regen_label(Some(Ecosystem::Python)),
            "Environment setup"
        );
        assert_eq!(ecosystem_regen_label(Some(Ecosystem::Go)), "Recompile");
        assert_eq!(ecosystem_regen_label(None), "Rebuild");
    }
}
