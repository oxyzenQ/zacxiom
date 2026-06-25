// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — v8.3
//!
//! Recommendation engine for safe cleanup actions.
//! Transforms Zacxiom from "What is this path?" into
//! "What cleanup action is safe and recommended?".
//!
//! CRITICAL: This module NEVER deletes anything.
//! No filesystem mutations. No `rm`. Recommendation only.

use crate::color;
use crate::discovery::{self, Ecosystem};
use crate::display::human_size;
use crate::engine::{self, types::RiskLevel, Category, ClassificationResult};
use crate::impact;
use crate::ownership;
use std::fs;
use std::path::Path;

/// A cleanup recommendation for a given path.
///
/// This is a read-only advisory. It never triggers deletion.
#[derive(Debug, Clone)]
pub struct CleanupPlan {
    /// Is this path safe to clean?
    pub safe_to_clean: bool,
    /// Risk level for cleanup.
    pub risk_level: RiskLevel,
    /// Estimated reclaimable space in bytes.
    pub estimated_reclaimable_bytes: u64,
    /// Human-readable recommendation.
    pub recommendation: String,
    /// How to regenerate the content after cleaning.
    pub regeneration: String,
    /// Suggested ecosystem-aware cleanup commands (never raw `rm -rf`).
    pub suggested_commands: Vec<String>,
    /// Additional notes and caveats.
    pub notes: Vec<String>,
    /// If unsafe, suggest safer child directories that actually exist.
    pub safer_alternatives: Vec<String>,
}

/// Generate a cleanup plan for the given path.
///
/// Consumes outputs from classification (v8.0), ownership (v8.1),
/// and impact (v8.2) without duplicating their logic.
pub fn plan(path: &Path) -> CleanupPlan {
    let mut eng = engine::classify(path);
    boost_confidence_from_discovery(&mut eng);

    let ownership = ownership::detect_project_ownership(path);
    let impact_analysis = impact::analyze_impact(path, &eng);

    let safe_to_clean = determine_safety(path, &eng, &ownership);
    let risk_level = compute_risk(&eng, &impact_analysis);
    let estimated_reclaimable_bytes = compute_size(path);
    let (recommendation, regeneration, suggested_commands) =
        build_ecosystem_recommendation(path, &eng, &ownership);
    let notes = build_notes(&eng, &ownership, &impact_analysis);
    let safer_alternatives = if safe_to_clean {
        Vec::new()
    } else {
        find_safer_children(path, &eng, &ownership)
    };

    CleanupPlan {
        safe_to_clean,
        risk_level,
        estimated_reclaimable_bytes,
        recommendation,
        regeneration,
        suggested_commands,
        notes,
        safer_alternatives,
    }
}

/// Boost confidence when project ownership is discovered.
fn boost_confidence_from_discovery(eng: &mut ClassificationResult) {
    if let Some(project) = discovery::find_project_for_path(&eng.path) {
        if eng.confidence_score < 95 {
            eng.confidence_score = (eng.confidence_score + 10).min(99);
        }
        let reason = format!(
            "Project ownership discovered: {} ({})",
            project.name,
            project.ecosystem.display()
        );
        if !eng.confidence_reasons.contains(&reason) {
            eng.confidence_reasons.push(reason);
        }
    }
}

/// Determine if a path is safe to clean based on its category and ownership.
fn determine_safety(
    target: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
) -> bool {
    // Unsafe categories — never clean
    match eng.category {
        Category::SystemBinary
        | Category::SystemConfiguration
        | Category::SystemData
        | Category::VirtualFilesystem
        | Category::SecurityCredential
        | Category::UserHomeRoot
        | Category::UserDocument
        | Category::UserMedia
        | Category::UserDesktop
        | Category::ShellConfiguration
        | Category::ApplicationConfiguration
        | Category::EnvironmentFile
        | Category::ProjectWorkspace
        | Category::SourceDirectory
        | Category::BuildManifest
        | Category::ProjectAsset
        | Category::InstalledSoftware => {
            return false;
        }
        _ => {}
    }

    // If ownership says this is a project root or configuration, it's unsafe
    if let Some(om) = ownership {
        match om.evidence.ownership_type {
            ownership::OwnershipType::ProjectRoot | ownership::OwnershipType::Configuration => {
                return false;
            }
            ownership::OwnershipType::ContainedInsideProject => {
                // Check if target contains source code indicators
                let pstr = target.to_string_lossy().to_lowercase();
                if pstr.ends_with("/src") || pstr.contains("/src/") {
                    return false;
                }
            }
            _ => {}
        }
    }

    // Safe categories — cache and build artifacts
    eng.category.is_cleanable()
}

/// Compute risk level from engine classification and impact analysis.
fn compute_risk(eng: &ClassificationResult, impact_analysis: &impact::ImpactAnalysis) -> RiskLevel {
    // Map engine risk level directly for protected/critical categories
    if eng.category.is_protected() || matches!(eng.category, Category::UserHomeRoot) {
        return RiskLevel::Critical;
    }

    // Map from impact level to our risk level
    match impact_analysis.level {
        impact::ImpactLevel::Critical => RiskLevel::Critical,
        impact::ImpactLevel::High => RiskLevel::High,
        impact::ImpactLevel::Medium => RiskLevel::Moderate,
        impact::ImpactLevel::Low => {
            if eng.category.is_cleanable() {
                RiskLevel::Low
            } else {
                RiskLevel::Moderate
            }
        }
    }
}

/// Compute the estimated size of a path in bytes.
fn compute_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }

    // Directory — walk and sum file sizes (read-only, no mutation)
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                total += fs::metadata(&entry_path).map(|m| m.len()).unwrap_or(0);
            } else if entry_path.is_dir() {
                total += compute_size(&entry_path);
            }
        }
    }
    total
}

/// Build ecosystem-aware recommendations.
/// Prefers ecosystem commands over raw `rm -rf`.
fn build_ecosystem_recommendation(
    path: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
) -> (String, String, Vec<String>) {
    let ecosystem = ownership.as_ref().map(|om| om.project.ecosystem);
    let path_str = path.to_string_lossy().to_lowercase();
    let path_last = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // ── Build artifacts ──
    if eng.category == Category::BuildCache {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Remove build artifacts.".into(),
                "cargo build".into(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => {
                let cmd = if path_last == "node_modules" {
                    "npm install"
                } else {
                    "npm run build"
                };
                (
                    "Remove dependencies and reinstall if needed.".into(),
                    "npm install".into(),
                    vec![cmd.into()],
                )
            }
            Some(Ecosystem::Go) => (
                "Remove Go build cache.".into(),
                "go build ./...".into(),
                vec!["go clean -cache".into()],
            ),
            _ => {
                // Generic build output
                let regen = if !eng.regenerated_by.is_empty() {
                    eng.regenerated_by.clone()
                } else {
                    "Rebuild the project".into()
                };
                ("Remove build output.".into(), regen, Vec::new())
            }
        };
    }

    // ── Generated content ──
    if eng.category == Category::GeneratedContent {
        let regen = if !eng.regenerated_by.is_empty() {
            eng.regenerated_by.clone()
        } else {
            "Regenerate from source".into()
        };
        return ("Remove generated content.".into(), regen, Vec::new());
    }

    // ── Cache categories ──
    if matches!(
        eng.category,
        Category::Cache | Category::BrowserCache | Category::CacheRegistry
    ) {
        let recommendation = match eng.category {
            Category::BrowserCache => "Clear browser cache.".into(),
            Category::CacheRegistry => "Remove package download cache.".into(),
            _ => "Clear application cache.".into(),
        };

        let regen = if !eng.regenerated_by.is_empty() {
            eng.regenerated_by.clone()
        } else {
            "Automatic — regenerated on next use".into()
        };

        return (recommendation, regen, Vec::new());
    }

    // ── Temporary files ──
    if eng.category == Category::TemporaryFile {
        return (
            "Remove temporary files.".into(),
            "Automatic — recreated by applications as needed".into(),
            Vec::new(),
        );
    }

    // ── Dependency sources (smart-cleanable) ──
    if matches!(
        eng.category,
        Category::DependencySource | Category::DownloadedArtifact
    ) {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Remove downloaded dependency sources.".into(),
                "cargo build (re-downloads dependencies)".into(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => (
                "Remove downloaded packages.".into(),
                "npm install (re-downloads packages)".into(),
                vec!["npm cache clean --force".into()],
            ),
            Some(Ecosystem::Python) => (
                "Remove downloaded packages.".into(),
                "pip install (re-downloads packages)".into(),
                Vec::new(),
            ),
            _ => (
                "Remove downloaded dependency sources.".into(),
                "Re-download via package manager".into(),
                Vec::new(),
            ),
        };
    }

    // ── Toolchain (smart-cleanable) ──
    if matches!(
        eng.category,
        Category::ToolchainManager | Category::ToolchainInstallation
    ) {
        return (
            "Remove toolchain. Requires reinstall to restore.".into(),
            "Reinstall via toolchain manager (rustup, nvm, etc.)".into(),
            Vec::new(),
        );
    }

    // ── Python-specific: __pycache__ ──
    if path_last == "__pycache__" || path_str.contains("/__pycache__/") {
        return (
            "Remove Python bytecode cache.".into(),
            "Automatic".into(),
            vec!["python -m compileall".into()],
        );
    }

    // ── Application data ──
    if matches!(
        eng.category,
        Category::ApplicationData | Category::DockerStorage | Category::GameData
    ) {
        return (
            "Review before cleaning — may contain user data or state.".into(),
            "Not easily regenerable — may require reconfiguration".into(),
            Vec::new(),
        );
    }

    // ── AI model cache ──
    if eng.category == Category::AIModelCache {
        return (
            "Remove AI model cache.".into(),
            "Re-download on next use".into(),
            Vec::new(),
        );
    }

    // ── Unsafe categories ──
    if matches!(
        eng.category,
        Category::ProjectWorkspace | Category::SourceDirectory
    ) {
        return ("Do not delete this path.".into(), String::new(), Vec::new());
    }

    if matches!(
        eng.category,
        Category::BuildManifest | Category::ProjectAsset
    ) {
        return (
            "Do not delete — project definition file.".into(),
            String::new(),
            Vec::new(),
        );
    }

    if matches!(
        eng.category,
        Category::SystemBinary
            | Category::SystemConfiguration
            | Category::SystemData
            | Category::VirtualFilesystem
            | Category::SecurityCredential
    ) {
        return (
            "Do not delete — system-critical path.".into(),
            "OS reinstall required".into(),
            Vec::new(),
        );
    }

    if matches!(
        eng.category,
        Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile
    ) {
        return (
            "Do not delete — configuration file.".into(),
            "Must recreate manually".into(),
            Vec::new(),
        );
    }

    if matches!(
        eng.category,
        Category::UserDocument | Category::UserMedia | Category::UserDesktop
    ) {
        return (
            "Do not delete — user-created content.".into(),
            "Irreplaceable without backup".into(),
            Vec::new(),
        );
    }

    if eng.category == Category::InstalledSoftware {
        return (
            "Do not delete — user-installed software.".into(),
            "Must reinstall explicitly".into(),
            Vec::new(),
        );
    }

    // ── Unknown fallback ──
    (
        "Cannot determine safety — manual review recommended.".into(),
        String::new(),
        Vec::new(),
    )
}

/// Build additional notes about the cleanup plan.
fn build_notes(
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
    impact_analysis: &impact::ImpactAnalysis,
) -> Vec<String> {
    let mut notes = Vec::new();

    // Add confidence note
    if eng.confidence_score < 50 {
        notes.push(format!(
            "Low classification confidence ({}%). Manual review advised.",
            eng.confidence_score
        ));
    }

    // Add ownership context
    if let Some(om) = ownership {
        notes.push(format!(
            "Owned by project: {} ({})",
            om.project.name,
            om.project.ecosystem.display()
        ));
        if om.evidence.confidence >= 80 {
            notes.push(format!("Ownership confidence: {}%", om.evidence.confidence));
        }
    }

    // Add impact context
    match impact_analysis.level {
        impact::ImpactLevel::Low => {
            if eng.category.is_cleanable() {
                notes.push("Next build may take longer.".into());
            }
        }
        impact::ImpactLevel::Medium => {
            notes.push("Re-download requires network access.".into());
        }
        _ => {}
    }

    // Add affected entity info for medium+ impact
    if !impact_analysis.affected.is_empty() && impact_analysis.level >= impact::ImpactLevel::Medium
    {
        let critical: Vec<_> = impact_analysis
            .affected
            .iter()
            .filter(|a| a.is_critical)
            .collect();
        if !critical.is_empty() {
            let names: Vec<&str> = critical.iter().map(|a| a.name.as_str()).collect();
            notes.push(format!("Critical dependencies: {}", names.join(", ")));
        }
    }

    notes
}

/// For unsafe paths (project roots, source dirs), find safer child directories
/// that actually exist on the filesystem.
fn find_safer_children(
    path: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
) -> Vec<String> {
    // Only suggest alternatives for project-level unsafe paths
    let is_project_level = matches!(
        eng.category,
        Category::ProjectWorkspace | Category::SourceDirectory
    ) || matches!(
        eng.category,
        Category::BuildManifest | Category::ProjectAsset
    );

    // Also check ownership — a temp dir may be Unknown category but
    // ownership engine detects it as a ProjectRoot.
    let is_project_by_ownership = ownership.as_ref().is_some_and(|om| {
        matches!(
            om.evidence.ownership_type,
            ownership::OwnershipType::ProjectRoot
        )
    });

    if !is_project_level && !is_project_by_ownership {
        return Vec::new();
    }

    let common_cleanable = [
        "target",
        "build",
        "dist",
        ".cache",
        "node_modules",
        "__pycache__",
        ".next",
        ".nuxt",
        "coverage",
        ".gradle",
        ".m2",
    ];

    let mut found = Vec::new();

    if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if common_cleanable.contains(&name.as_str()) {
                    found.push(format!("{}/", name));
                }
            }
        }
    }

    found
}

// ═══════════════════════════════════════════════════════════════
// Rendering
// ═══════════════════════════════════════════════════════════════

/// Render a cleanup plan to a formatted string matching Zacxiom visual style.
pub fn render_plan(plan: &CleanupPlan, path: &Path) -> String {
    let mut out = String::new();

    // Section header
    out.push_str(&color::section_header("CLEANUP PLAN"));

    // Target
    let display_path = if path.to_string_lossy().ends_with('/') {
        path.to_string_lossy().to_string()
    } else {
        format!("{}/", path.display())
    };
    out.push_str(&format!("  {:<20} {}\n", "Target:", display_path));

    // Risk
    out.push_str(&format!(
        "  {:<20} {}\n",
        "Risk:",
        plan.risk_level.display()
    ));

    // Space
    out.push_str(&format!(
        "  {:<20} {}\n",
        "Space:",
        human_size(plan.estimated_reclaimable_bytes)
    ));

    // Safe to clean
    let safe_label = if plan.safe_to_clean {
        color::purple("YES")
    } else {
        "NO".to_string()
    };
    out.push_str(&format!("  {:<20} {}\n", "Safe To Clean:", safe_label));

    out.push('\n');

    // Recommendation
    if !plan.recommendation.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Recommendation:", plan.recommendation
        ));
    }

    // Regeneration
    if !plan.regeneration.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Regeneration:", plan.regeneration
        ));
    }

    // Suggested Commands (for safe paths)
    if !plan.suggested_commands.is_empty() {
        out.push_str("  Suggested Commands:\n");
        for cmd in &plan.suggested_commands {
            out.push_str(&format!("    {}\n", cmd));
        }
    }

    // Expected result
    if plan.safe_to_clean {
        out.push_str(&format!(
            "  {:<20} Reclaim approximately {}\n",
            "Expected Result:",
            human_size(plan.estimated_reclaimable_bytes)
        ));
    } else {
        // Reason / Expected result for unsafe
        if !plan.recommendation.is_empty() {
            out.push_str(&format!("  {:<20} {}\n", "Reason:", plan.recommendation));
        }
        out.push_str(&format!(
            "  {:<20} Prevent permanent data loss.\n",
            "Expected Result:",
        ));
    }

    // Safer alternatives (for unsafe paths)
    if !plan.safer_alternatives.is_empty() {
        out.push('\n');
        out.push_str("  Consider reviewing:\n");
        for alt in &plan.safer_alternatives {
            out.push_str(&format!("    - {}\n", alt));
        }
    }

    // Notes
    if !plan.notes.is_empty() {
        out.push('\n');
        for note in &plan.notes {
            out.push_str(&format!("  {}\n", note));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_rust_project() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test-plan\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        // Write a dummy file in target so we can measure size
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
        let target_path = root.join("target");
        let plan = plan(&target_path);
        assert!(plan.safe_to_clean);
        assert_eq!(plan.risk_level, RiskLevel::Low);
        // Should suggest cargo clean for Rust projects
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
        let src_path = root.join("src");
        let plan = plan(&src_path);
        assert!(!plan.safe_to_clean);
    }

    #[test]
    fn test_plan_cargo_toml_unsafe() {
        let (_dir, root) = setup_rust_project();
        let manifest_path = root.join("Cargo.toml");
        let plan = plan(&manifest_path);
        assert!(!plan.safe_to_clean);
    }

    #[test]
    fn test_plan_node_modules_safe() {
        let (_dir, root) = setup_node_project();
        let nm_path = root.join("node_modules");
        let plan = plan(&nm_path);
        assert!(plan.safe_to_clean);
    }

    #[test]
    fn test_plan_no_deletion() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target");
        let target_before = target_path.exists();
        let _ = plan(&target_path);
        let target_after = target_path.exists();
        // Plan must not modify filesystem
        assert_eq!(target_before, target_after);
    }

    #[test]
    fn test_plan_size_estimation() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target");
        let plan = plan(&target_path);
        // We wrote 1024 bytes in target/debug/test-binary
        assert!(plan.estimated_reclaimable_bytes > 0);
    }

    #[test]
    fn test_plan_safer_alternatives_for_project_root() {
        let (_dir, root) = setup_rust_project();
        let plan = plan(&root);
        assert!(!plan.safe_to_clean);
        // Project root with target/ should suggest target/ as alternative
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
        let target_path = root.join("target");
        let plan = plan(&target_path);
        // Must NEVER suggest rm -rf
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
        // Must prefer cargo clean
        assert!(plan
            .suggested_commands
            .iter()
            .any(|c| c.contains("cargo clean")));
    }

    #[test]
    fn test_plan_node_uses_npm_commands() {
        let (_dir, root) = setup_node_project();
        let nm_path = root.join("node_modules");
        let plan = plan(&nm_path);
        // Should not suggest rm -rf
        for cmd in &plan.suggested_commands {
            assert!(!cmd.contains("rm -rf"));
            assert!(!cmd.contains("rm "));
        }
    }

    #[test]
    fn test_render_plan_safe_output() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target");
        let plan = plan(&target_path);
        let output = render_plan(&plan, &target_path);
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
    fn test_plan_notes_not_empty_for_known_paths() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target");
        let plan = plan(&target_path);
        // Should have at least ownership note since it's in a Rust project
        assert!(!plan.notes.is_empty());
    }

    #[test]
    fn test_plan_nonexistent_path() {
        let path = Path::new("/tmp/zacxiom-test-nonexistent-xyz");
        let plan = plan(path);
        assert_eq!(plan.estimated_reclaimable_bytes, 0);
    }

    #[test]
    fn test_plan_safer_alternatives_only_existing() {
        let (_dir, root) = setup_rust_project();
        // Remove the target directory so it shouldn't be suggested
        let _ = fs::remove_dir_all(root.join("target"));
        // Recreate it empty so the project root is still valid
        fs::create_dir(root.join("target")).unwrap();
        // Remove node_modules — it shouldn't appear
        // Only target/ should be suggested
        let plan = plan(&root);
        for alt in &plan.safer_alternatives {
            assert!(root.join(alt.trim_end_matches('/')).exists());
        }
    }
}
