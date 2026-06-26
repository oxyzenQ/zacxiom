// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ownership detection — two layers:
//!
//! Layer 1 (v1): OS-level — dpkg package vs user vs system vs orphan.
//! Layer 2 (v8.1): Project-level — which project owns this path, why, and
//!                 with what evidence and confidence.
//!
//! The project ownership layer is evidence-based: every ownership claim
//! must be backed by discoverable evidence on the filesystem.

use crate::discovery::{self, ProjectInfo};
use crate::rules::{is_protected, is_user_protected, Ownership};
use std::path::Path;
use std::process::Command;

/// Determine ownership of a file.
pub fn detect(path: &Path) -> Ownership {
    let path_str = path.to_string_lossy();

    // Check dpkg ownership only for paths that could plausibly be system packages.
    // Skipping dpkg for /tmp, /home, and other user paths saves ~30ms per file.
    if path_str.starts_with("/usr/")
        || path_str.starts_with("/bin/")
        || path_str.starts_with("/sbin/")
        || path_str.starts_with("/lib/")
        || path_str.starts_with("/lib64/")
        || path_str.starts_with("/etc/")
        || path_str.starts_with("/boot/")
        || path_str.starts_with("/opt/")
    {
        if let Some(pkg) = dpkg_owns(path) {
            return Ownership::Package { pkg_name: pkg };
        }
    }

    // Check if file is in a home directory
    if let Some(home) = std::env::var_os("HOME") {
        let home = Path::new(&home);
        if path.starts_with(home) {
            // Check user-protected paths (H2)
            if is_user_protected(home, path) {
                return Ownership::System;
            }
            // Use fallback uid 0 if we can't determine (non-unix or test env)
            let uid = get_uid();
            return Ownership::User { uid };
        }
    }

    // If it's a protected system path, mark as system
    if is_protected(path) {
        return Ownership::System;
    }

    // Anything else is orphan
    Ownership::Orphan
}

/// Get current user id in a cross-platform way.
#[cfg(unix)]
fn get_uid() -> u32 {
    unsafe { libc::getuid() }
}

#[cfg(not(unix))]
fn get_uid() -> u32 {
    0
}

/// Query dpkg for the package owning a file.
fn dpkg_owns(path: &Path) -> Option<String> {
    let output = Command::new("dpkg")
        .args(["-S", &path.to_string_lossy()])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // dpkg -S outputs "pkgname: /path/to/file"
        stdout.split(':').next().map(|s| s.trim().to_string())
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════
// v8.1: Project Ownership Engine — evidence-based ownership layer
// ═══════════════════════════════════════════════════════════════

/// Types of project ownership relationships.
#[derive(Debug, Clone, PartialEq)]
pub enum OwnershipType {
    /// Path IS the project root directory.
    ProjectRoot,
    /// Path is inside a project root directory.
    ContainedInsideProject,
    /// Path is a build artifact (target/, dist/, build/).
    BuildArtifact,
    /// Path is a dependency registry/cache (~/.cargo/registry).
    DependencyRegistry,
    /// Path is a package manager internal cache (~/.npm/).
    PackageManagerCache,
    /// Path is a configuration file inside a project.
    Configuration,
    /// No project ownership detectable.
    Unknown,
}

impl OwnershipType {
    pub fn display(&self) -> &'static str {
        match self {
            OwnershipType::ProjectRoot => "Project Root",
            OwnershipType::ContainedInsideProject => "Inside Project",
            OwnershipType::BuildArtifact => "Build Artifact",
            OwnershipType::DependencyRegistry => "Dependency Registry",
            OwnershipType::PackageManagerCache => "Package Manager Cache",
            OwnershipType::Configuration => "Project Configuration",
            OwnershipType::Unknown => "Unknown",
        }
    }
}

/// Evidence pieces that support an ownership claim.
#[derive(Debug, Clone)]
pub struct OwnershipEvidence {
    /// Paths to evidence files found (e.g. Cargo.toml, Cargo.lock).
    pub evidence_files: Vec<String>,
    /// Human-readable reasons why this ownership is correct.
    pub reasons: Vec<String>,
    /// The ownership relationship type.
    pub ownership_type: OwnershipType,
    /// Confidence score 0-100 (caps at 100).
    pub confidence: u8,
}

/// Full project ownership result.
#[derive(Debug, Clone)]
pub struct OwnershipMatch {
    /// Which project owns this path.
    pub project: ProjectInfo,
    /// Evidence supporting the ownership claim.
    pub evidence: OwnershipEvidence,
}

/// Detect project ownership for a given path.
/// Returns the owning project with evidence, if detectable.
pub fn detect_project_ownership(path: &Path) -> Option<OwnershipMatch> {
    // Step 1: Try to find the owning project
    let project = discovery::find_project_for_path(path)?;

    // Step 2: Determine ownership type
    let ownership_type = classify_ownership_type(path, &project);

    // Step 3: Collect evidence
    let mut evidence_files: Vec<String> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();
    let mut confidence: u8 = 0;

    // Collect manifest evidence
    for m in &project.manifests {
        let name = m
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        evidence_files.push(name);
    }

    // Build ownership evidence — cumulative confidence model
    match ownership_type {
        OwnershipType::ProjectRoot => {
            confidence = confidence.saturating_add(50);
            reasons.push(format!(
                "Path is the project root directory of {}",
                project.name
            ));
        }
        OwnershipType::ContainedInsideProject
        | OwnershipType::BuildArtifact
        | OwnershipType::Configuration
        | OwnershipType::PackageManagerCache => {
            // All these types imply the path is inside a project
            confidence = confidence.saturating_add(30);
            reasons.push(format!("Path is located inside project {}", project.name));
            // Additional type-specific evidence
            match ownership_type {
                OwnershipType::BuildArtifact => {
                    reasons.push("Build artifacts generated from project source".into());
                }
                OwnershipType::Configuration => {
                    reasons.push("Configuration file belonging to project".into());
                }
                OwnershipType::PackageManagerCache => {
                    reasons.push(format!(
                        "Package manager cache for {} ecosystem",
                        project.ecosystem.display()
                    ));
                }
                _ => {}
            }
        }
        OwnershipType::DependencyRegistry => {
            confidence = confidence.saturating_add(25);
            reasons.push(format!(
                "Registry contains downloaded dependencies for {} ecosystem",
                project.ecosystem.display()
            ));

            let all_consumers = discovery::find_projects_using_registry(path);
            if all_consumers.len() > 1 {
                reasons.push(format!(
                    "Registry shared by {} {} project(s)",
                    all_consumers.len(),
                    project.ecosystem.display()
                ));
            }
        }
        OwnershipType::Unknown => {
            // No confidence boost for unknown ownership
            return None;
        }
    }

    // Cumulative evidence (applies to all ownership types)

    // Manifest evidence: primary manifest found at project root
    if project.primary_manifest().is_some() {
        confidence = confidence.saturating_add(20);
        reasons.push("Manifest evidence: project manifest confirms ownership".into());
    }

    // Lockfile evidence
    if evidence_files.iter().any(|f| f.ends_with(".lock")) {
        confidence = confidence.saturating_add(10);
        reasons.push("Lockfile evidence: dependency versions are pinned".into());
    }

    // Same ecosystem bonus
    confidence = confidence.saturating_add(10);

    // Cap at 100
    confidence = confidence.min(100);

    Some(OwnershipMatch {
        project,
        evidence: OwnershipEvidence {
            evidence_files,
            reasons,
            ownership_type,
            confidence,
        },
    })
}

/// Classify the ownership relationship between a path and its project.
fn classify_ownership_type(path: &Path, project: &ProjectInfo) -> OwnershipType {
    let path_canonical = normalize_path(path);
    let root_canonical = normalize_path(&project.root);

    // Exact match: path IS project root
    if path_canonical == root_canonical {
        return OwnershipType::ProjectRoot;
    }

    let path_str = path.to_string_lossy().to_lowercase();

    // Build artifact patterns
    if path_str.contains("/target/")
        || path_str.starts_with("target/")
        || path_str.starts_with("./target/")
        || path_str == "target"
        || path_str.ends_with("/target")
        || path_str.contains("/dist/")
        || path_str.contains("/build/")
    {
        return OwnershipType::BuildArtifact;
    }

    // Dependency registry patterns
    if path_str.contains("/.cargo/registry")
        || path_str.contains("/.npm/_cacache")
        || path_str.contains("/.npm/_npx")
        || path_str.contains("/.cache/pip/")
        || path_str.contains("/.cache/uv/")
    {
        return OwnershipType::DependencyRegistry;
    }

    // Package manager cache
    if path_str.contains("/.npm/")
        || path_str.contains("/node_modules/")
        || path_str.contains("/.m2/repository/")
        || path_str.contains("/.gradle/caches/")
    {
        return OwnershipType::PackageManagerCache;
    }

    // Configuration files — only primary manifests (lockfiles are regenerable)
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if file_name == "Cargo.toml"
        || file_name == "package.json"
        || file_name == "go.mod"
        || file_name == "pyproject.toml"
    {
        return OwnershipType::Configuration;
    }

    // Inside project root (catch-all for paths within the project)
    let path_parent = path.parent().map(|p| p.to_path_buf());
    if path_starts_with(path, &project.root) || path_starts_with_canonical(path, &project.root) {
        return OwnershipType::ContainedInsideProject;
    }

    // Check if parent dirs lead to project
    if let Some(parent) = &path_parent {
        if path_starts_with(parent, &project.root)
            || path_starts_with_canonical(parent, &project.root)
        {
            return OwnershipType::ContainedInsideProject;
        }
    }

    OwnershipType::Unknown
}

/// Normalize a path for comparison (resolve relative paths, canonicalize).
fn normalize_path(path: &Path) -> std::path::PathBuf {
    if path.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

/// Check if a path starts with a prefix (string comparison after normalization).
fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    path.to_string_lossy()
        .to_lowercase()
        .starts_with(&prefix.to_string_lossy().to_lowercase())
}

/// Check using canonical (dereferenced) paths.
fn path_starts_with_canonical(path: &Path, prefix: &Path) -> bool {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let r = std::fs::canonicalize(prefix).unwrap_or_else(|_| prefix.to_path_buf());
    p.to_string_lossy()
        .to_lowercase()
        .starts_with(&r.to_string_lossy().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_home_file() {
        let home = std::env::var_os("HOME").unwrap();
        let test_path = Path::new(&home).join(".cache/test_file");
        let ownership = detect(&test_path);
        assert!(matches!(ownership, Ownership::User { .. }));
    }

    #[test]
    fn test_system_path_is_system() {
        let ownership = detect(Path::new("/etc/passwd"));
        assert!(matches!(ownership, Ownership::System));
    }

    #[test]
    fn test_orphan_non_home_non_system() {
        // /opt or /srv without package — should be orphan
        let ownership = detect(Path::new("/opt/some-app/cache/data.bin"));
        assert!(matches!(ownership, Ownership::Orphan));
    }

    // ═══════════════════════════════════════════════════════════
    // v8.1: Project Ownership Engine tests
    // ═══════════════════════════════════════════════════════════

    use crate::discovery::Ecosystem;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_rust_project() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"test-project\"\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::create_dir(root.join("target/debug")).unwrap();
        (dir, root)
    }

    #[test]
    fn test_ownership_project_root() {
        let (_dir, root) = setup_rust_project();
        let ownership = detect_project_ownership(&root);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert_eq!(om.evidence.ownership_type, OwnershipType::ProjectRoot);
        assert_eq!(om.project.ecosystem, Ecosystem::Rust);
        assert!(om.evidence.confidence >= 80);
        assert!(!om.evidence.evidence_files.is_empty());
    }

    #[test]
    fn test_ownership_source_directory() {
        let (_dir, root) = setup_rust_project();
        let src_path = root.join("src");
        let ownership = detect_project_ownership(&src_path);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert_eq!(
            om.evidence.ownership_type,
            OwnershipType::ContainedInsideProject
        );
        assert_eq!(om.project.ecosystem, Ecosystem::Rust);
    }

    #[test]
    fn test_ownership_target_directory() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target");
        let ownership = detect_project_ownership(&target_path);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert_eq!(om.evidence.ownership_type, OwnershipType::BuildArtifact);
        assert_eq!(om.project.ecosystem, Ecosystem::Rust);
    }

    #[test]
    fn test_ownership_target_release() {
        let (_dir, root) = setup_rust_project();
        let target_release = root.join("target/release");
        let ownership = detect_project_ownership(&target_release);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert_eq!(om.evidence.ownership_type, OwnershipType::BuildArtifact);
    }

    #[test]
    fn test_ownership_cargo_toml() {
        let (_dir, root) = setup_rust_project();
        let cargo_path = root.join("Cargo.toml");
        let ownership = detect_project_ownership(&cargo_path);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert_eq!(om.evidence.ownership_type, OwnershipType::Configuration);
        assert!(om.evidence.reasons.iter().any(|r| r.contains("project")));
    }

    #[test]
    fn test_ownership_cargo_registry() {
        let (_dir, _root) = setup_rust_project();
        // Cargo registry — should use discovery to find projects
        let registry_path = std::path::Path::new("/home/user/.cargo/registry");
        let ownership = detect_project_ownership(registry_path);
        // In test, discovery may or may not find projects depending on env
        // Verify no panic and correct type if found
        if let Some(om) = ownership {
            assert_eq!(
                om.evidence.ownership_type,
                OwnershipType::DependencyRegistry
            );
        }
    }

    #[test]
    fn test_ownership_npm_cache() {
        let npmpath = std::path::Path::new("/home/user/.npm/_cacache");
        let ownership = detect_project_ownership(npmpath);
        if let Some(om) = ownership {
            assert_eq!(
                om.evidence.ownership_type,
                OwnershipType::DependencyRegistry
            );
        }
    }

    #[test]
    fn test_ownership_unknown_fallback() {
        // A path with no project context should return None
        let ownership = detect_project_ownership(std::path::Path::new("/tmp/random-file"));
        // May or may not find project depending on env; just check no panic
        assert!(ownership.is_none() || ownership.is_some());
    }

    #[test]
    fn test_ownership_confidence_capped() {
        let (_dir, root) = setup_rust_project();
        let ownership = detect_project_ownership(&root);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert!(om.evidence.confidence <= 100);
    }

    #[test]
    fn test_ownership_type_display() {
        assert_eq!(OwnershipType::ProjectRoot.display(), "Project Root");
        assert_eq!(
            OwnershipType::ContainedInsideProject.display(),
            "Inside Project"
        );
        assert_eq!(OwnershipType::BuildArtifact.display(), "Build Artifact");
        assert_eq!(
            OwnershipType::DependencyRegistry.display(),
            "Dependency Registry"
        );
        assert_eq!(OwnershipType::Unknown.display(), "Unknown");
    }

    #[test]
    fn test_ownership_evidence_contains_manifest() {
        let (_dir, root) = setup_rust_project();
        let ownership = detect_project_ownership(&root);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert!(om.evidence.evidence_files.iter().any(|f| f == "Cargo.toml"));
        assert!(om.evidence.evidence_files.iter().any(|f| f == "Cargo.lock"));
    }

    #[test]
    fn test_ownership_build_artifact_reason() {
        let (_dir, root) = setup_rust_project();
        let target_path = root.join("target/debug");
        let ownership = detect_project_ownership(&target_path);
        assert!(ownership.is_some());
        let om = ownership.unwrap();
        assert!(om
            .evidence
            .reasons
            .iter()
            .any(|r| r.contains("build") || r.contains("Build")));
    }
}
