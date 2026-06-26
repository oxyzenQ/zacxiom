// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — Notes, expected results, and safer alternatives.

use std::fs;
use std::path::Path;

use crate::display::human_size;
use crate::engine::{Category, ClassificationResult};
use crate::impact;
use crate::ownership;

/// P6: Build contextual expected result — path-aware wording.
pub(crate) fn build_expected_result(
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
        let path_str = _path.to_string_lossy().to_lowercase();
        if path_str.contains(".ssh") {
            return "SSH access continues to work.".into();
        }
        if path_str.contains(".gnupg") || path_str.contains("gpg") {
            return "Encrypted communications and signatures continue to work.".into();
        }
        return "Authentication continues to work.".into();
    }
    if matches!(
        eng.category,
        Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile
    ) {
        return match eng.category {
            Category::ShellConfiguration => {
                "Shell retains all user-defined behavior and prompt settings."
            }
            Category::ApplicationConfiguration => {
                "Applications retain their customized configuration."
            }
            Category::EnvironmentFile => "Applications retain their runtime configuration.",
            _ => "Configuration is preserved.",
        }
        .into();
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
pub(crate) fn build_notes(
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

    // Add ownership context with evidence
    if let Some(om) = ownership {
        notes.push(format!(
            "Owned by project: {} ({})",
            om.project.name,
            om.project.ecosystem.display()
        ));
        notes.push(format!("Ownership confidence: {}%", om.evidence.confidence));
        if !om.evidence.evidence_files.is_empty() {
            let evidence_list: Vec<String> = om
                .evidence
                .evidence_files
                .iter()
                .map(|f| format!("  + {}", f))
                .collect();
            notes.push(format!("Evidence:\n{}", evidence_list.join("\n")));
        }
    }

    // Add impact context — category-aware wording
    match impact_analysis.level {
        impact::ImpactLevel::Low => {
            if eng.category.is_cleanable() {
                match eng.category {
                    Category::BuildCache => {
                        notes.push("Next build may take longer.".into());
                    }
                    Category::BrowserCache => {
                        notes.push("Browser will re-download resources while browsing.".into());
                    }
                    Category::CacheRegistry => {
                        notes.push("Files will be downloaded again if needed.".into());
                    }
                    Category::Cache | Category::TemporaryFile => {
                        notes.push("Applications may recreate cache during next launch.".into());
                    }
                    _ => {
                        notes.push("Next build may take longer.".into());
                    }
                }
            }
        }
        impact::ImpactLevel::Medium => match eng.category {
            Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile => {
                notes.push("User preferences may need to be configured again.".into());
            }
            Category::DependencySource | Category::DownloadedArtifact => {
                notes.push("Dependencies will be reinstalled when required.".into());
            }
            Category::ToolchainManager | Category::ToolchainInstallation => {
                notes.push("Toolchain must be reinstalled to restore.".into());
            }
            _ => {
                notes.push("Re-download requires network access.".into());
            }
        },
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
pub(crate) fn find_safer_children(
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
