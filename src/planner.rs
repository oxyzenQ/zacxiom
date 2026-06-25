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
    /// Human-readable recommendation (what to do).
    pub recommendation: String,
    /// Why this recommendation (distinct from recommendation — no duplication).
    pub reason: String,
    /// How to regenerate the content after cleaning.
    pub regeneration: String,
    /// Suggested ecosystem-aware cleanup commands (never raw `rm -rf`).
    pub suggested_commands: Vec<String>,
    /// Additional notes and caveats.
    pub notes: Vec<String>,
    /// If unsafe, suggest safer child directories that actually exist.
    pub safer_alternatives: Vec<String>,
    /// Contextual expected result — path-aware wording.
    pub expected_result: String,
}

/// System-critical paths that must never be planned.
static DANGEROUS_PATHS: &[&str] = &[
    "/", "/home", "/usr", "/etc", "/var", "/boot", "/root", "/sys", "/proc", "/dev", "/run",
];

/// Check if a path is a dangerous system path that must be blocked.
fn is_dangerous_system_path(path: &Path) -> Option<&'static str> {
    let raw = path.to_string_lossy();
    if raw == "/" {
        return Some("/");
    }
    let normalized = raw.trim_end_matches('/');
    DANGEROUS_PATHS.iter().find(|b| normalized == **b).copied()
}

/// Error returned when a dangerous path is blocked.
pub struct BlockedPath {
    pub path: String,
    pub reason: String,
    pub suggestions: Vec<&'static str>,
}

/// Check if a path is blocked and return the block info.
pub fn check_path_blocked(path: &Path) -> Result<(), BlockedPath> {
    if let Some(blocked) = is_dangerous_system_path(path) {
        return Err(BlockedPath {
            path: blocked.to_string(),
            reason: "System-critical path.".into(),
            suggestions: vec![
                "zacxiom scan",
                "zacxiom plan ~/.cache",
                "zacxiom plan ~/Downloads",
            ],
        });
    }
    Ok(())
}

/// Render a blocked path error message.
pub fn render_blocked(blocked: &BlockedPath) -> String {
    let mut out = String::new();
    out.push_str(&color::section_header("ERROR"));
    out.push_str(&format!(
        "  Refusing to create cleanup plan for:\n\n    {}\n\n",
        blocked.path
    ));
    out.push_str(&format!("  Reason:\n    {}\n\n", blocked.reason));
    out.push_str(
        "  Why:\n    Cleanup recommendations on this path may be misleading or dangerous.\n\n",
    );
    out.push_str("  Try:\n");
    for suggestion in &blocked.suggestions {
        out.push_str(&format!("    {}\n", suggestion));
    }
    out
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
    let (recommendation, reason, regeneration, suggested_commands) =
        build_ecosystem_recommendation(path, &eng, &ownership);
    let notes = build_notes(&eng, &ownership, &impact_analysis);
    let safer_alternatives = if safe_to_clean {
        Vec::new()
    } else {
        find_safer_children(path, &eng, &ownership)
    };
    let expected_result = build_expected_result(
        path,
        &eng,
        &ownership,
        safe_to_clean,
        &estimated_reclaimable_bytes,
    );

    CleanupPlan {
        safe_to_clean,
        risk_level,
        estimated_reclaimable_bytes,
        recommendation,
        reason,
        regeneration,
        suggested_commands,
        notes,
        safer_alternatives,
        expected_result,
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
            let entrypath = entry.path();
            if entrypath.is_file() {
                total += fs::metadata(&entrypath).map(|m| m.len()).unwrap_or(0);
            } else if entrypath.is_dir() {
                total += compute_size(&entrypath);
            }
        }
    }
    total
}

/// Build ecosystem-aware recommendations.
/// Prefers ecosystem commands over raw `rm -rf`.
/// Returns (recommendation, reason, regeneration, suggested_commands).
/// P4: recommendation and reason ALWAYS contain different information.
fn build_ecosystem_recommendation(
    path: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
) -> (String, String, String, Vec<String>) {
    let ecosystem = ownership.as_ref().map(|om| om.project.ecosystem);
    let path_str = path.to_string_lossy().to_lowercase();
    let path_last = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // ── P2: Project root / CRITICAL — suggest child cleanup, never root deletion ──
    let is_project_root = ownership.as_ref().is_some_and(|om| {
        matches!(
            om.evidence.ownership_type,
            ownership::OwnershipType::ProjectRoot
        )
    }) || matches!(
        eng.category,
        Category::ProjectWorkspace | Category::SourceDirectory
    );

    if is_project_root && !eng.category.is_cleanable() {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Review build artifacts instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => (
                "Review dependencies instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["npm install".into()],
            ),
            Some(Ecosystem::Python) => (
                "Review virtual environment and cache instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["python -m venv .venv".into()],
            ),
            Some(Ecosystem::Go) => (
                "Review build cache instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["go clean -cache".into()],
            ),
            None => (
                "Review cleanable subdirectories instead.".into(),
                "This directory contains non-regenerable content.".into(),
                String::new(),
                Vec::new(),
            ),
        };
    }

    // ── Build artifacts ──
    if eng.category == Category::BuildCache {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Remove build artifacts.".into(),
                "Compiled binaries and intermediate objects are fully regenerable from source."
                    .into(),
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
                    "Installed packages are declared in package.json and re-downloadable.".into(),
                    "npm install".into(),
                    vec![cmd.into()],
                )
            }
            Some(Ecosystem::Go) => (
                "Remove Go build cache.".into(),
                "Compiled binaries are fully regenerable from source.".into(),
                "go build ./...".into(),
                vec!["go clean -cache".into()],
            ),
            _ => {
                let regen = if !eng.regenerated_by.is_empty() {
                    eng.regenerated_by.clone()
                } else {
                    "Rebuild the project".into()
                };
                let reason = if !eng.deletion_impact.is_empty() {
                    eng.deletion_impact.clone()
                } else {
                    "Build output is regenerable from source and dependencies.".into()
                };
                ("Remove build output.".into(), reason, regen, Vec::new())
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
        return (
            "Remove generated content.".into(),
            "Auto-generated from source — no manual work lost.".into(),
            regen,
            Vec::new(),
        );
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
        let reason = match eng.category {
            Category::BrowserCache => {
                "Web resources are re-downloaded automatically while browsing.".into()
            }
            Category::CacheRegistry => {
                "Package metadata is rebuilt on next package operation.".into()
            }
            _ => "Disposable data — applications regenerate it on next use.".into(),
        };
        let regen = if !eng.regenerated_by.is_empty() {
            eng.regenerated_by.clone()
        } else {
            "Automatic — regenerated on next use".into()
        };
        return (recommendation, reason, regen, Vec::new());
    }

    // ── Temporary files ──
    if eng.category == Category::TemporaryFile {
        return (
            "Remove temporary files.".into(),
            "Designed by the OS and applications to be disposable.".into(),
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
                "Crates are re-downloaded from crates.io on next build.".into(),
                "cargo build (re-downloads dependencies)".into(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => (
                "Remove downloaded packages.".into(),
                "Packages are re-downloadable from the npm registry.".into(),
                "npm install (re-downloads packages)".into(),
                vec!["npm cache clean --force".into()],
            ),
            Some(Ecosystem::Python) => (
                "Remove downloaded packages.".into(),
                "Packages are re-downloadable from PyPI.".into(),
                "pip install (re-downloads packages)".into(),
                Vec::new(),
            ),
            _ => (
                "Remove downloaded dependency sources.".into(),
                "Dependencies are re-downloadable from their registry.".into(),
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
            "Installed compiler/runtime — not auto-regenerated.".into(),
            "Reinstall via toolchain manager (rustup, nvm, etc.)".into(),
            Vec::new(),
        );
    }

    // ── Python-specific: __pycache__ ──
    if path_last == "__pycache__" || path_str.contains("/__pycache__/") {
        return (
            "Remove Python bytecode cache.".into(),
            "Bytecode is recompiled automatically when modules are imported.".into(),
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
            "Application state may include preferences, saves, or session data.".into(),
            "Not easily regenerable — may require reconfiguration".into(),
            Vec::new(),
        );
    }

    // ── AI model cache ──
    if eng.category == Category::AIModelCache {
        return (
            "Remove AI model cache.".into(),
            "Downloaded model weights are re-downloadable from the model hub.".into(),
            "Re-download on next use".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: project-level (already handled above, but for non-root project paths) ──
    if matches!(
        eng.category,
        Category::BuildManifest | Category::ProjectAsset
    ) {
        return (
            "Keep this file.".into(),
            "Project definition file — defines dependencies and build configuration.".into(),
            String::new(),
            Vec::new(),
        );
    }

    // ── Unsafe: system ──
    if matches!(
        eng.category,
        Category::SystemBinary
            | Category::SystemConfiguration
            | Category::SystemData
            | Category::VirtualFilesystem
            | Category::SecurityCredential
    ) {
        let reason = match eng.category {
            Category::SystemBinary => {
                "Operating system requires these binaries to function.".into()
            }
            Category::SystemConfiguration => {
                "System services depend on these configuration files.".into()
            }
            Category::SecurityCredential => {
                "Authentication credentials cannot be recovered once deleted.".into()
            }
            _ => "Operating system infrastructure — required for system stability.".into(),
        };
        return (
            "Preserve system infrastructure.".into(),
            reason,
            "OS reinstall required".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: configuration ──
    if matches!(
        eng.category,
        Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile
    ) {
        let reason = match eng.category {
            Category::ShellConfiguration => {
                "Custom aliases, functions, and shell settings would be lost.".into()
            }
            Category::ApplicationConfiguration => {
                "Application settings and preferences would reset to defaults.".into()
            }
            Category::EnvironmentFile => {
                "Environment variables and secrets would need to be reconfigured.".into()
            }
            _ => "Configuration contains user customizations.".into(),
        };
        return (
            "Preserve application settings.".into(),
            reason,
            "Must recreate manually".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: user content ──
    if matches!(
        eng.category,
        Category::UserDocument | Category::UserMedia | Category::UserDesktop
    ) {
        return (
            "Protect user-created content.".into(),
            "User-authored files are irreplaceable without a backup.".into(),
            "Irreplaceable without backup".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: installed software ──
    if eng.category == Category::InstalledSoftware {
        return (
            "Preserve installed software.".into(),
            "User-installed tools would need to be reinstalled individually.".into(),
            "Must reinstall explicitly".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: home root ──
    if eng.category == Category::UserHomeRoot {
        return (
            "Protect home directory.".into(),
            "Contains all personal files, projects, and configurations.".into(),
            "Not regenerable — user must recreate from scratch".into(),
            Vec::new(),
        );
    }

    // ── Unknown fallback ──
    (
        "Manual review recommended.".into(),
        "Classification uncertain — cannot determine safety automatically.".into(),
        String::new(),
        Vec::new(),
    )
}

/// P6: Build contextual expected result — path-aware wording.
fn build_expected_result(
    _path: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
    safe_to_clean: bool,
    size: &u64,
) -> String {
    if safe_to_clean {
        return format!("Reclaim approximately {}", human_size(*size));
    }

    // Contextual unsafe expected results
    if matches!(
        eng.category,
        Category::ProjectWorkspace | Category::SourceDirectory
    ) || ownership.as_ref().is_some_and(|om| {
        matches!(
            om.evidence.ownership_type,
            ownership::OwnershipType::ProjectRoot
        )
    }) {
        return "Protect project source code.".into();
    }
    if matches!(
        eng.category,
        Category::SystemBinary
            | Category::SystemConfiguration
            | Category::SystemData
            | Category::VirtualFilesystem
    ) {
        return "Preserve system infrastructure.".into();
    }
    if matches!(eng.category, Category::SecurityCredential) {
        return "Preserve authentication credentials.".into();
    }
    if matches!(
        eng.category,
        Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile
    ) {
        return "Preserve application settings.".into();
    }
    if eng.category == Category::BuildCache {
        return "Reclaim build artifact storage.".into();
    }
    if matches!(
        eng.category,
        Category::Cache | Category::BrowserCache | Category::CacheRegistry
    ) {
        return "Reclaim disposable cache storage.".into();
    }
    if matches!(
        eng.category,
        Category::UserDocument | Category::UserMedia | Category::UserDesktop
    ) {
        return "Protect user-created content.".into();
    }
    if eng.category == Category::UserHomeRoot {
        return "Protect all personal data.".into();
    }
    if eng.category == Category::InstalledSoftware {
        return "Preserve installed tools.".into();
    }

    "Prevent permanent data loss.".into()
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

// ── Rendering ──

/// Render a cleanup plan to a formatted string matching Zacxiom visual style.
pub fn render_plan(plan: &CleanupPlan, path: &Path) -> String {
    let mut out = String::new();

    out.push_str(&color::section_header("CLEANUP PLAN"));

    let displaypath = if path.to_string_lossy().ends_with('/') {
        path.to_string_lossy().to_string()
    } else {
        format!("{}/", path.display())
    };
    out.push_str(&format!("  {:<20} {}\n", "Target:", displaypath));
    out.push_str(&format!(
        "  {:<20} {}\n",
        "Risk:",
        plan.risk_level.display()
    ));
    out.push_str(&format!(
        "  {:<20} {}\n",
        "Space:",
        human_size(plan.estimated_reclaimable_bytes)
    ));

    let safe_label = if plan.safe_to_clean {
        color::purple("YES")
    } else {
        "NO".to_string()
    };
    out.push_str(&format!("  {:<20} {}\n", "Safe To Clean:", safe_label));
    out.push('\n');

    // P4: Recommendation and Reason are always different fields
    if !plan.recommendation.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Recommendation:", plan.recommendation
        ));
    }
    if !plan.reason.is_empty() {
        out.push_str(&format!("  {:<20} {}\n", "Reason:", plan.reason));
    }
    if !plan.regeneration.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Regeneration:", plan.regeneration
        ));
    }
    if !plan.suggested_commands.is_empty() {
        out.push_str("  Suggested Commands:\n");
        for cmd in &plan.suggested_commands {
            out.push_str(&format!("    {}\n", cmd));
        }
    }

    // P6: Contextual expected result
    if !plan.expected_result.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Expected Result:", plan.expected_result
        ));
    }

    if !plan.safer_alternatives.is_empty() {
        out.push('\n');
        out.push_str("  Consider reviewing:\n");
        for alt in &plan.safer_alternatives {
            out.push_str(&format!("    - {}\n", alt));
        }
    }

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
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
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
        // Plan must not modify filesystem
        assert_eq!(target_before, target_after);
    }

    #[test]
    fn test_plan_size_estimation() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
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
        let targetpath = root.join("target");
        let plan = plan(&targetpath);
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
        let nmpath = root.join("node_modules");
        let plan = plan(&nmpath);
        // Should not suggest rm -rf
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
        // Should have at least ownership note since it's in a Rust project
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
        // P2: Must suggest child cleanup, never root deletion
        assert!(p.recommendation.contains("Review build artifacts instead"));
        assert!(p
            .suggested_commands
            .iter()
            .any(|c| c.contains("cargo clean")));
        // Must NOT say "delete" in recommendation
        assert!(!p.recommendation.to_lowercase().contains("delete"));
    }

    #[test]
    fn test_plan_rust_project_in_tmp() {
        // P3: /tmp/rust-test + Cargo.toml -> ProjectWorkspace (not TemporaryFile)
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"tmp-rust\"\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir(root.join("target")).unwrap();

        let eng = crate::engine::classify(&root);
        // Should be ProjectWorkspace, NOT TemporaryFile
        assert_eq!(
            eng.category,
            Category::ProjectWorkspace,
            "Project in /tmp should be ProjectWorkspace, got {:?}",
            eng.category
        );
    }

    #[test]
    fn test_plan_node_project_in_tmp() {
        // P3: /tmp/node-test + package.json -> ProjectWorkspace
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
        // Safe path: recommendation and reason should differ
        assert_ne!(p.recommendation, p.reason);

        let p2 = plan(&root);
        // Unsafe path: recommendation and reason should differ
        assert_ne!(p2.recommendation, p2.reason);
    }

    #[test]
    fn test_p6_contextual_expected_result() {
        let (_dir, root) = setup_rust_project();
        let targetpath = root.join("target");
        let p = plan(&targetpath);
        // Safe build cache should say "Reclaim build artifact storage"
        assert!(p.expected_result.contains("Reclaim approximately"));

        let p2 = plan(&root);
        // Unsafe project root should say "Protect project source code"
        assert_eq!(p2.expected_result, "Protect project source code.");
    }

    #[test]
    fn test_config_dir_classification() {
        // P5: ~/.config should be ApplicationConfiguration
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
}
