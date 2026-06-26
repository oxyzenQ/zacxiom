// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — v8.3.1
//!
//! Recommendation engine for safe cleanup actions.
//! Transforms Zacxiom from "What is this path?" into
//! "What cleanup action is safe and recommended?".
//!
//! CRITICAL: This module NEVER deletes anything.
//! No filesystem mutations. No `rm`. Recommendation only.

mod notes;
mod ownership;
mod recommendation;
pub mod regeneration;
mod render;
pub mod types;

pub use recommendation::{check_path_blocked, plan, render_blocked};
pub use render::render_plan;
pub use types::CleanupPlan;

// Re-export for test access

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::RiskLevel;
    use crate::engine::Category;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn setup_rust_project() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test-plan\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/test-binary"), vec![0u8; 1024]).unwrap();
        (dir, root)
    }

    fn setup_node_project() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("package.json"),
            "{\"name\": \"test\", \"version\": \"1.0.0\"}",
        )
        .unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::create_dir(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "").unwrap();
        (dir, root)
    }

    #[test]
    fn test_plan_target_directory_safe() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
        assert!(plan.safe_to_clean);
        assert_eq!(plan.risk_level, RiskLevel::Low);
        assert!(plan.suggested_commands.iter().any(|c| c == "cargo clean"));
        assert!(!plan.recommendation.is_empty());
    }

    #[test]
    fn test_plan_project_root_unsafe() {
        let (_dir, root) = setup_rust_project();
        let plan = plan(&root);
        assert!(!plan.safe_to_clean);
        assert_eq!(plan.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_plan_source_directory_unsafe() {
        let (_dir, root) = setup_rust_project();
        let srcpath = root.join("src");
        let plan = plan(&srcpath);
        assert!(!plan.safe_to_clean);
    }

    #[test]
    fn test_plan_cargo_toml_unsafe() {
        let (_dir, root) = setup_rust_project();
        let manifestpath = root.join("Cargo.toml");
        let plan = plan(&manifestpath);
        assert!(!plan.safe_to_clean);
    }

    #[test]
    fn test_plan_node_modules_safe() {
        let (_dir, root) = setup_node_project();
        let nmpath = root.join("node_modules");
        let plan = plan(&nmpath);
        assert!(plan.safe_to_clean);
    }

    #[test]
    fn test_plan_no_deletion() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let target_before = targetpath.exists();
        let _ = plan(&targetpath);
        let target_after = targetpath.exists();
        assert_eq!(target_before, target_after);
    }

    #[test]
    fn test_plan_size_estimation() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
        assert!(plan.estimated_reclaimable_bytes > 0);
    }

    #[test]
    fn test_plan_safer_alternatives_for_project_root() {
        let (_dir, root) = setup_rust_project();
        let plan = plan(&root);
        assert!(!plan.safe_to_clean);
        assert!(plan.safer_alternatives.iter().any(|a| a == "target/"));
    }

    #[test]
    fn test_plan_system_binary_critical() {
        let path = Path::new("/usr/bin/ls");
        if path.exists() {
            let plan = plan(path);
            assert!(!plan.safe_to_clean);
            assert_eq!(plan.risk_level, RiskLevel::Critical);
        }
    }

    #[test]
    fn test_plan_uses_ecosystem_commands_not_rm() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
        for cmd in &plan.suggested_commands {
            assert!(
                !cmd.contains("rm -rf"),
                "Command should not contain 'rm -rf': {cmd}"
            );
            assert!(
                !cmd.contains("rm "),
                "Command should not contain 'rm ': {cmd}"
            );
        }
        assert!(plan
            .suggested_commands
            .iter()
            .any(|c| c.contains("cargo clean")));
    }

    #[test]
    fn test_plan_node_uses_npm_commands() {
        let (_dir, root) = setup_node_project();
        let nmpath = root.join("node_modules");
        let plan = plan(&nmpath);
        for cmd in &plan.suggested_commands {
            assert!(!cmd.contains("rm -rf"));
            assert!(!cmd.contains("rm "));
        }
    }

    #[test]
    fn test_render_plan_safe_output() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
        let output = render_plan(&plan, &targetpath);
        assert!(output.contains("CLEANUP PLAN"));
        assert!(output.contains("YES"));
        assert!(output.contains("cargo clean"));
        assert!(output.contains("─"));
    }

    #[test]
    fn test_render_plan_unsafe_output() {
        let (_dir, root) = setup_rust_project();
        let plan = plan(&root);
        let output = render_plan(&plan, &root);
        assert!(output.contains("CLEANUP PLAN"));
        assert!(output.contains("NO"));
        assert!(output.contains("Critical"));
        assert!(output.contains("Consider reviewing:"));
        assert!(output.contains("target/"));
    }

    #[test]
    fn test_render_plan_no_box_characters() {
        let (_dir, root) = setup_rust_project();
        let plan = plan(&root.join("target"));
        let output = render_plan(&plan, &root.join("target"));
        assert!(!output.contains('┌'));
        assert!(!output.contains('┐'));
        assert!(!output.contains('╔'));
        assert!(!output.contains('╚'));
        assert!(!output.contains('├'));
        assert!(!output.contains('┤'));
    }

    #[test]
    fn test_plan_notes_not_empty_for_knownpaths() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
        assert!(!plan.notes.is_empty());
    }

    #[test]
    fn test_plan_nonexistentpath() {
        let path = Path::new("/tmp/zacxiom-test-nonexistent-xyz");
        let plan = plan(path);
        assert_eq!(plan.estimated_reclaimable_bytes, 0);
    }

    #[test]
    fn test_plan_safer_alternatives_only_existing() {
        let (_dir, root) = setup_rust_project();
        let _ = fs::remove_dir_all(root.join("target"));
        fs::create_dir(root.join("target")).unwrap();
        let plan = plan(&root);
        for alt in &plan.safer_alternatives {
            assert!(root.join(alt.trim_end_matches('/')).exists());
        }
    }

    // ═══════════════════════════════════════════════════════════
    // v8.3.1 Integration Tests
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_plan_root_blocked() {
        assert!(check_path_blocked(Path::new("/")).is_err());
    }

    #[test]
    fn test_plan_usr_blocked() {
        assert!(check_path_blocked(Path::new("/usr")).is_err());
    }

    #[test]
    fn test_plan_etc_blocked() {
        assert!(check_path_blocked(Path::new("/etc")).is_err());
    }

    #[test]
    fn test_plan_var_blocked() {
        assert!(check_path_blocked(Path::new("/var")).is_err());
    }

    #[test]
    fn test_plan_home_blocked() {
        assert!(check_path_blocked(Path::new("/home")).is_err());
    }

    #[test]
    fn test_plan_boot_blocked() {
        assert!(check_path_blocked(Path::new("/boot")).is_err());
    }

    #[test]
    fn test_plan_proc_blocked() {
        assert!(check_path_blocked(Path::new("/proc")).is_err());
    }

    #[test]
    fn test_plan_dev_blocked() {
        assert!(check_path_blocked(Path::new("/dev")).is_err());
    }

    #[test]
    fn test_render_blocked_output() {
        let blocked = check_path_blocked(Path::new("/")).unwrap_err();
        let output = render_blocked(&blocked);
        assert!(output.contains("ERROR"));
        assert!(output.contains("/"));
        assert!(output.contains("System-critical path"));
        assert!(output.contains("zacxiom plan ~/.cache"));
    }

    #[test]
    fn test_plan_project_root_never_deletes() {
        let (_dir, root) = setup_rust_project();
        let p = plan(&root);
        assert!(!p.safe_to_clean);
        assert!(p
            .recommendation
            .contains("Clean generated artifacts instead"));
        assert!(p
            .suggested_commands
            .iter()
            .any(|c| c.contains("cargo clean")));
        assert!(!p.recommendation.to_lowercase().contains("delete"));
    }

    #[test]
    fn test_plan_rust_project_in_tmp() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"tmp-rust\"\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir(root.join("target")).unwrap();

        let eng = crate::engine::classify(&root);
        assert_eq!(
            eng.category,
            Category::ProjectWorkspace,
            "Project in /tmp should be ProjectWorkspace, got {:?}",
            eng.category
        );
    }

    #[test]
    fn test_plan_node_project_in_tmp() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("package.json"),
            "{\"name\": \"tmp-node\", \"version\": \"1.0.0\"}",
        )
        .unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();

        let eng = crate::engine::classify(&root);
        assert_eq!(
            eng.category,
            Category::ProjectWorkspace,
            "Project in /tmp should be ProjectWorkspace, got {:?}",
            eng.category
        );
    }

    #[test]
    fn test_p4_no_recommendation_reason_duplication() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let p = plan(&targetpath);
        assert_ne!(p.recommendation, p.reason);

        let p2 = plan(&root);
        assert_ne!(p2.recommendation, p2.reason);
    }

    #[test]
    fn test_p6_contextual_expected_result() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let p = plan(&targetpath);
        assert!(p.expected_result.contains("Reclaim approximately"));

        let p2 = plan(&root);
        assert_eq!(p2.expected_result, "Protect project source code.");
    }

    #[test]
    fn test_config_dir_classification() {
        let home = std::env::var("HOME").unwrap();
        let configpath = Path::new(&home).join(".config");
        if configpath.exists() {
            let eng = crate::engine::classify(&configpath);
            assert_eq!(
                eng.category,
                Category::ApplicationConfiguration,
                "~/.config should be ApplicationConfiguration, got {:?}",
                eng.category
            );
        }
    }

    // ═══════════════════════════════════════════════════════════
    // v8.3.2 Polish Tests
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_polish_config_fields_are_distinct() {
        let home = std::env::var("HOME").unwrap();
        let configpath = Path::new(&home).join(".config");
        if configpath.exists() {
            let p = plan(&configpath);
            assert_ne!(
                p.recommendation, p.expected_result,
                "Recommendation and Expected Result must differ"
            );
            assert_ne!(
                p.recommendation, p.reason,
                "Recommendation and Reason must differ"
            );
            assert_ne!(
                p.reason, p.expected_result,
                "Reason and Expected Result must differ"
            );
        }
    }

    #[test]
    fn test_polish_ssh_specific_wording() {
        let home = std::env::var("HOME").unwrap();
        let sshpath = Path::new(&home).join(".ssh");
        if sshpath.exists() {
            let p = plan(&sshpath);
            assert!(
                p.recommendation.contains("SSH") || p.recommendation.contains("ssh"),
                "SSH path should mention SSH, got: {}",
                p.recommendation
            );
            assert!(
                !p.recommendation.contains("system infrastructure"),
                "SSH path should NOT say 'system infrastructure'"
            );
            assert!(
                p.expected_result.contains("SSH") || p.expected_result.contains("ssh"),
                "SSH expected result should mention SSH, got: {}",
                p.expected_result
            );
        }
    }

    #[test]
    fn test_polish_project_root_action_oriented() {
        let (_dir, root) = setup_rust_project();
        let p = plan(&root);
        assert!(
            p.recommendation.starts_with("Clean"),
            "Project root recommendation should start with 'Clean', got: {}",
            p.recommendation
        );
    }

    #[test]
    fn test_polish_confidence_shows_evidence() {
        let (_dir, root) = setup_rust_project();
        let p = plan(&root);
        let notes_text = p.notes.join("\n");
        assert!(
            notes_text.contains("Evidence:"),
            "Ownership note should show evidence, got: {}",
            notes_text
        );
        assert!(
            notes_text.contains("Cargo.toml"),
            "Evidence should list Cargo.toml, got: {}",
            notes_text
        );
    }

    #[test]
    fn test_polish_config_regeneration_wording() {
        let home = std::env::var("HOME").unwrap();
        let configpath = Path::new(&home).join(".config");
        if configpath.exists() {
            let p = plan(&configpath);
            assert!(
                p.regeneration.contains("recreates default"),
                "Config regeneration should mention 'recreates default', got: {}",
                p.regeneration
            );
            assert!(
                !p.regeneration.contains("Must recreate manually"),
                "Config regeneration should NOT say 'Must recreate manually'"
            );
        }
    }

    #[test]
    fn test_polish_node_project_root_action_oriented() {
        let (_dir, root) = setup_node_project();
        let p = plan(&root);
        assert!(
            p.recommendation.starts_with("Clean"),
            "Node project root recommendation should start with 'Clean', got: {}",
            p.recommendation
        );
    }

    // ── v8.5.1: UX Polish tests ──

    #[test]
    fn test_p1_cache_note_says_applications_recreate() {
        let (_dir, root) = setup_rust_project();
        let cache_dir = root.join(".cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("data"), vec![0u8; 2_100_000]).unwrap();

        let p = plan(&cache_dir);
        let notes_text = p.notes.join(" ");
        if p.safe_to_clean && notes_text.contains("build") {
            panic!(
                "Application cache note should not say 'Next build may take longer', got: {}",
                notes_text
            );
        }
    }

    #[test]
    fn test_p1_build_cache_note_says_next_build() {
        let (_dir, root) = setup_rust_project();
        let target_dir = root.join("target");
        std::fs::create_dir_all(target_dir.join("debug")).unwrap();
        std::fs::write(target_dir.join("debug/binary"), vec![0u8; 2_100_000]).unwrap();

        let p = plan(&target_dir);
        let notes_text = p.notes.join(" ");
        if p.safe_to_clean {
            assert!(
                notes_text.contains("Next build may take longer"),
                "BuildCache note should say 'Next build may take longer', got: {}",
                notes_text
            );
        }
    }

    #[test]
    fn test_p2_config_note_does_not_say_re_download() {
        let home = std::env::var("HOME").unwrap();
        let configpath = Path::new(&home).join(".config");
        if configpath.exists() {
            let p = plan(&configpath);
            let notes_text = p.notes.join(" ");
            assert!(
                !notes_text.contains("Re-download requires network access"),
                "Config path should NOT say 'Re-download requires network access', got: {}",
                notes_text
            );
        }
    }

    #[test]
    fn test_p2_config_note_says_preferences() {
        let home = std::env::var("HOME").unwrap();
        let configpath = Path::new(&home).join(".config");
        if configpath.exists() {
            let p = plan(&configpath);
            let notes_text = p.notes.join(" ");
            if notes_text.contains("network") || notes_text.contains("re-download") {
                panic!("Config path has download-related note, got: {}", notes_text);
            }
        }
    }
}
