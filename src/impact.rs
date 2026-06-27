// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Impact Analysis Engine — v8.2
//!
//! Determines what would be affected if a path were deleted.
//! Reports consequences, risk levels, and affected projects.
//!
//! No cleanup actions. No deletion execution. No graph traversal.
//! Impact reporting only.

use crate::discovery;
use crate::engine::ClassificationResult;
use crate::ownership::{self, OwnershipType};
use std::path::Path;

/// Severity of impact if a path is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactLevel {
    /// Negligible impact — regenerable, no data loss.
    Low,
    /// Moderate impact — re-downloadable but time/bandwidth cost.
    Medium,
    /// Significant impact — reinstall required, data may be lost.
    High,
    /// Severe impact — irreplaceable, permanent data loss.
    Critical,
}

impl ImpactLevel {
    pub fn display(&self) -> &'static str {
        match self {
            ImpactLevel::Low => "LOW",
            ImpactLevel::Medium => "MEDIUM",
            ImpactLevel::High => "HIGH",
            ImpactLevel::Critical => "CRITICAL",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ImpactLevel::Low => "Safe — fully regenerable with no lasting consequences",
            ImpactLevel::Medium => "Re-downloadable — requires network access and time to restore",
            ImpactLevel::High => "Significant — reinstall required, potential data loss",
            ImpactLevel::Critical => "Severe — irreplaceable, permanent data loss",
        }
    }
}

/// Who or what is affected by deleting this path.
#[derive(Debug, Clone)]
pub struct AffectedEntity {
    /// Name of the affected entity (project name, tool name, etc.)
    pub name: String,
    /// How this entity is affected.
    pub relationship: String,
    /// Is this entity critical to the user?
    pub is_critical: bool,
}

/// Full impact analysis result for a path.
#[derive(Debug, Clone)]
pub struct ImpactAnalysis {
    /// What is the impact level of deleting this path?
    pub level: ImpactLevel,
    /// Who/what would be affected?
    pub affected: Vec<AffectedEntity>,
    /// What regenerates automatically if deleted?
    pub regenerates: String,
    /// What breaks and must be manually restored?
    pub breaks: String,
    /// Human-readable consequence summary.
    pub consequence: String,
    /// Confidence in this analysis (0-100).
    pub confidence: u8,
}

/// Analyze the impact of deleting a given path.
/// Uses classification results and ownership data to determine consequences.
pub fn analyze_impact(path: &Path, eng: &ClassificationResult) -> ImpactAnalysis {
    let ownership = ownership::detect_project_ownership(path);
    let mut affected: Vec<AffectedEntity> = Vec::new();

    // Step 1: Determine base impact from classification
    let (mut level, mut regenerates, mut breaks, mut confidence) =
        classify_base_impact(eng, &mut affected);

    // Step 2: Enrich with ownership data
    if let Some(ref om) = ownership {
        affected.push(AffectedEntity {
            name: om.project.name.clone(),
            relationship: format!("Owning project — {}", om.evidence.ownership_type.display()),
            is_critical: matches!(
                om.evidence.ownership_type,
                OwnershipType::ProjectRoot | OwnershipType::Configuration
            ),
        });

        // Override level based on ownership type for better accuracy
        match om.evidence.ownership_type {
            OwnershipType::BuildArtifact => {
                level = level.min(ImpactLevel::Low);
                regenerates = eng.regenerated_by.clone();
            }
            OwnershipType::DependencyRegistry => {
                let consumers = discovery::find_projects_using_registry(path);
                for consumer in consumers {
                    if consumer.name != om.project.name {
                        affected.push(AffectedEntity {
                            name: consumer.name.clone(),
                            relationship: format!(
                                "Consuming project — {} ecosystem",
                                consumer.ecosystem.display()
                            ),
                            is_critical: false,
                        });
                    }
                }
                level = level.min(ImpactLevel::Medium);
                if affected.len() > 1 {
                    breaks = format!(
                        "All {} consuming project(s) must re-download dependencies",
                        affected.len()
                    );
                }
            }
            OwnershipType::Configuration | OwnershipType::ProjectRoot => {
                level = ImpactLevel::Critical;
                regenerates =
                    "Not regenerable — must recreate from scratch or restore from backup".into();
            }
            OwnershipType::ContainedInsideProject => {
                let path_str = path.to_string_lossy().to_lowercase();
                if path_str.ends_with("/src") || path_str.contains("/src/") {
                    level = ImpactLevel::Critical;
                    regenerates = "Source code — irreplaceable without version control".into();
                }
            }
            _ => {}
        }

        confidence = confidence.saturating_add(om.evidence.confidence / 10);
        confidence = confidence.min(100);
    }

    // Step 3: Build consequence summary
    let consequence = build_consequence_summary(level, &affected, &regenerates, &breaks);

    ImpactAnalysis {
        level,
        affected,
        regenerates,
        breaks,
        consequence,
        confidence,
    }
}

/// Classify base impact from engine category alone (before ownership enrichment).
fn classify_base_impact(
    eng: &ClassificationResult,
    affected: &mut Vec<AffectedEntity>,
) -> (ImpactLevel, String, String, u8) {
    use crate::engine::Category;

    match eng.category {
        // Truly system-critical — OS infrastructure
        Category::SystemBinary
        | Category::SystemConfiguration
        | Category::SystemData
        | Category::VirtualFilesystem
        | Category::SecurityCredential => {
            affected.push(AffectedEntity {
                name: "Operating System".into(),
                relationship: "System-critical infrastructure".into(),
                is_critical: true,
            });
            (
                ImpactLevel::Critical,
                "Cannot be regenerated — OS reinstall required".into(),
                "System or installed applications may fail".into(),
                100,
            )
        }

        // User-critical but not system — project/workspace files
        Category::ProjectWorkspace
        | Category::SourceDirectory
        | Category::BuildManifest
        | Category::ProjectAsset
        | Category::UserHomeRoot
        | Category::UserDocument
        | Category::UserMedia
        | Category::UserDesktop => {
            affected.push(AffectedEntity {
                name: "User".into(),
                relationship: "User-created content — irreplaceable without backup".into(),
                is_critical: true,
            });
            (
                ImpactLevel::Critical,
                "Recoverable only from backup or version control".into(),
                "Permanent loss of user-authored content".into(),
                95,
            )
        }

        // Temporary files — not regenerated, but also not important
        Category::TemporaryFile => (
            ImpactLevel::Low,
            "Applications create new temporary files naturally during normal operation".into(),
            "No functional impact — applications create new temp files as needed".into(),
            95,
        ),

        cat if cat.is_cleanable() => {
            if eng.regenerable {
                (
                    ImpactLevel::Low,
                    eng.regenerated_by.clone(),
                    "No permanent damage — rebuilds or re-downloads automatically".into(),
                    85,
                )
            } else {
                (
                    ImpactLevel::Medium,
                    "May require manual intervention".into(),
                    "Re-download or rebuild required".into(),
                    60,
                )
            }
        }

        cat if cat.is_smart_cleanable() => (
            ImpactLevel::Medium,
            eng.regenerated_by.clone(),
            "Must re-download or reinstall. Offline builds may fail.".into(),
            70,
        ),

        _ => {
            if eng.regenerable {
                (
                    ImpactLevel::Low,
                    eng.regenerated_by.clone(),
                    eng.deletion_impact.clone(),
                    60,
                )
            } else {
                (
                    ImpactLevel::Medium,
                    "Not regenerable".into(),
                    eng.deletion_impact.clone(),
                    50,
                )
            }
        }
    }
}

/// Build a human-readable consequence summary.
fn build_consequence_summary(
    level: ImpactLevel,
    affected: &[AffectedEntity],
    _regenerates: &str,
    breaks: &str,
) -> String {
    let names: Vec<&str> = affected.iter().map(|a| a.name.as_str()).collect();
    let who = if names.is_empty() {
        String::new()
    } else {
        format!("Affects {}. ", names.join(", "))
    };
    let b = breaks.trim_end_matches('.');

    match level {
        ImpactLevel::Low => {
            format!("Minimal — {who}{b}.")
        }
        ImpactLevel::Medium => {
            format!("Moderate — {who}{b}.")
        }
        ImpactLevel::High => {
            format!("Significant — {who}{b}.")
        }
        ImpactLevel::Critical => {
            format!("CRITICAL — deleting this file or directory causes permanent data loss. {who}Recovery depends on backups or version control.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_rust_project() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test-proj\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        (dir, root)
    }

    #[test]
    fn test_impact_build_artifact_low() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target/debug");
        let eng = crate::engine::classify(&target_path);
        let analysis = analyze_impact(&target_path, &eng);
        assert_eq!(analysis.level, ImpactLevel::Low);
        // Build artifacts should affect some project (name varies with temp dir)
        assert!(!analysis.affected.is_empty());
    }

    #[test]
    fn test_impact_source_directory_critical() {
        let (_dir, root) = setup_rust_project();
        let src_path = root.join("src");
        let eng = crate::engine::classify(&src_path);
        let analysis = analyze_impact(&src_path, &eng);
        assert_eq!(analysis.level, ImpactLevel::Critical);
    }

    #[test]
    fn test_impact_cargo_toml_critical() {
        let (_dir, root) = setup_rust_project();
        let manifest_path = root.join("Cargo.toml");
        let eng = crate::engine::classify(&manifest_path);
        let analysis = analyze_impact(&manifest_path, &eng);
        assert_eq!(analysis.level, ImpactLevel::Critical);
    }

    #[test]
    fn test_impact_registry_medium() {
        let (_dir, _root) = setup_rust_project();
        let registry_path = std::path::Path::new("/home/user/.cargo/registry");
        let eng = crate::engine::classify(registry_path);
        let analysis = analyze_impact(registry_path, &eng);
        // Registry should be Low or Medium, not Critical
        assert!(analysis.level <= ImpactLevel::Medium);
    }

    #[test]
    fn test_impact_system_binary_critical() {
        let eng = crate::engine::classify(std::path::Path::new("/usr/bin/bash"));
        let analysis = analyze_impact(std::path::Path::new("/usr/bin/bash"), &eng);
        assert_eq!(analysis.level, ImpactLevel::Critical);
        assert_eq!(analysis.confidence, 100);
    }

    #[test]
    fn test_impact_cache_low() {
        let eng = crate::engine::classify(std::path::Path::new(
            "/home/user/.cache/BraveSoftware/Brave-Browser/Cache/data",
        ));
        let analysis = analyze_impact(
            std::path::Path::new("/home/user/.cache/BraveSoftware/Brave-Browser/Cache/data"),
            &eng,
        );
        assert_eq!(analysis.level, ImpactLevel::Low);
    }

    #[test]
    fn test_impact_level_display() {
        assert_eq!(ImpactLevel::Low.display(), "LOW");
        assert_eq!(ImpactLevel::Medium.display(), "MEDIUM");
        assert_eq!(ImpactLevel::High.display(), "HIGH");
        assert_eq!(ImpactLevel::Critical.display(), "CRITICAL");
    }

    #[test]
    fn test_impact_level_ordering() {
        assert!(ImpactLevel::Low < ImpactLevel::Medium);
        assert!(ImpactLevel::Medium < ImpactLevel::High);
        assert!(ImpactLevel::High < ImpactLevel::Critical);
    }

    #[test]
    fn test_impact_consequence_not_empty() {
        let (_dir, root) = setup_rust_project();
        let path = root.join("src");
        let eng = crate::engine::classify(&path);
        let analysis = analyze_impact(&path, &eng);
        assert!(!analysis.consequence.is_empty());
        assert!(!analysis.regenerates.is_empty());
    }

    #[test]
    fn test_impact_confidence_range() {
        let (_dir, root) = setup_rust_project();
        let path = root.join("target/debug");
        let eng = crate::engine::classify(&path);
        let analysis = analyze_impact(&path, &eng);
        assert!(analysis.confidence <= 100);
        assert!(analysis.confidence > 0);
    }

    #[test]
    fn test_impact_toolchain_not_critical() {
        let eng = crate::engine::classify(std::path::Path::new(
            "/home/user/.rustup/toolchains/stable-x86_64",
        ));
        let analysis = analyze_impact(
            std::path::Path::new("/home/user/.rustup/toolchains/stable-x86_64"),
            &eng,
        );
        // Toolchains are smart-cleanable, should be Medium not Critical
        assert!(analysis.level <= ImpactLevel::Medium);
    }
}
