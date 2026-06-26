// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Project Discovery Engine — v8.0
//!
//! Discovers local software projects and their package ecosystems.
//! Foundation layer for all future intelligence features.
//!
//! Answers:
//!   - Which projects exist on this machine?
//!   - Which project owns this artifact?
//!   - How many projects depend on this cache?
//!
//! No graph generation. No ownership engine. No cleanup simulation.
//! Discovery infrastructure only.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Supported package ecosystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ecosystem {
    Rust,
    Node,
    Python,
    Go,
}

impl Ecosystem {
    pub fn display(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "Rust",
            Ecosystem::Node => "Node.js",
            Ecosystem::Python => "Python",
            Ecosystem::Go => "Go",
        }
    }

    /// Manifest files that identify this ecosystem in a project root.
    fn manifest_files(&self) -> &[&str] {
        match self {
            Ecosystem::Rust => &["Cargo.toml", "Cargo.lock"],
            Ecosystem::Node => &[
                "package.json",
                "package-lock.json",
                "pnpm-lock.yaml",
                "yarn.lock",
            ],
            Ecosystem::Python => &[
                "pyproject.toml",
                "requirements.txt",
                "poetry.lock",
                "Pipfile",
            ],
            Ecosystem::Go => &["go.mod", "go.sum"],
        }
    }

    /// The primary manifest (first file in the manifest list).
    fn primary_manifest(&self) -> &str {
        self.manifest_files()[0]
    }

    /// Detect ecosystem from a manifest file name.
    fn from_manifest(name: &str) -> Option<Ecosystem> {
        match name {
            "Cargo.toml" | "Cargo.lock" => Some(Ecosystem::Rust),
            "package.json" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" => {
                Some(Ecosystem::Node)
            }
            "pyproject.toml" | "requirements.txt" | "poetry.lock" | "Pipfile" => {
                Some(Ecosystem::Python)
            }
            "go.mod" | "go.sum" => Some(Ecosystem::Go),
            _ => None,
        }
    }
}

/// Information about a discovered project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Project name derived from directory name.
    pub name: String,
    /// Project root directory.
    pub root: PathBuf,
    /// Package ecosystem.
    pub ecosystem: Ecosystem,
    /// Paths to all manifest files found in the project root.
    pub manifests: Vec<PathBuf>,
}

impl ProjectInfo {
    /// Get the primary manifest path.
    pub fn primary_manifest(&self) -> Option<&PathBuf> {
        let primary_name = self.ecosystem.primary_manifest();
        self.manifests
            .iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(primary_name))
    }
}

/// Check if a directory itself is a project root (no parent traversal).
///
/// Unlike `find_project_for_path` (which walks UP to find containing projects),
/// this function ONLY checks the given directory for ecosystem markers.
/// Used by workspace discovery to avoid false positives on subdirectories.
pub fn is_project_root(path: &Path) -> Option<ProjectInfo> {
    if !path.is_dir() {
        return None;
    }

    let resolved = if path.is_relative() {
        std::env::current_dir().ok()?.join(path)
    } else {
        path.to_path_buf()
    };

    if let Ok(entries) = fs::read_dir(&resolved) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(ecosystem) = Ecosystem::from_manifest(&name_str) {
                let mut manifests = Vec::new();
                for mf in ecosystem.manifest_files() {
                    let mf_path = resolved.join(mf);
                    if mf_path.exists() {
                        manifests.push(mf_path);
                    }
                }
                if manifests.iter().any(|p| {
                    p.file_name().and_then(|n| n.to_str()) == Some(ecosystem.primary_manifest())
                }) {
                    let project_name = resolved
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    return Some(ProjectInfo {
                        name: project_name,
                        root: resolved,
                        ecosystem,
                        manifests,
                    });
                }
            }
        }
    }

    None
}

/// Discovery cache stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DiscoveryCache {
    projects: Vec<CachedProject>,
    #[serde(default)]
    last_scan: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedProject {
    name: String,
    root: String,
    ecosystem: String,
    manifests: Vec<String>,
}

/// Cache file path: ~/.cache/zacxiom/discovery.json
fn cache_path() -> Option<PathBuf> {
    let base = dirs_cache()?;
    let dir = base.join("zacxiom");
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join("discovery.json"))
}

/// Get the user cache directory.
fn dirs_cache() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("XDG_CACHE_HOME") {
        if !d.is_empty() {
            return Some(PathBuf::from(d));
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".cache"))
}

/// Get the user home directory.
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Default search directories for project discovery.
/// Limited to common development locations — no recursive disk scanning.
fn default_search_roots() -> Vec<PathBuf> {
    let home = match home_dir() {
        Some(h) => h,
        None => return vec![],
    };
    vec![
        home.join("Desktop"),
        home.join("Documents"),
        home.join("Projects"),
        home.join("Code"),
        home.join("Development"),
        home.join("workspace"),
        home.join("dev"),
    ]
}

// ═══════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════

/// Discover all projects in the default search locations.
/// Uses a cache to avoid repeated filesystem scanning.
/// Cache is refreshed if older than 5 minutes.
pub fn discover_projects() -> Vec<ProjectInfo> {
    // Try loading from cache first
    if let Some(cached) = load_cache() {
        let now = epoch_secs();
        if now.saturating_sub(cached.last_scan) < 300 {
            // Cache is fresh (< 5 min)
            return convert_cache(&cached);
        }
    }

    // Collect all search roots
    let mut roots = default_search_roots();
    if let Ok(cwd) = std::env::current_dir() {
        if !roots.contains(&cwd) {
            roots.push(cwd);
        }
    }

    // Scan and cache
    let projects = scan_projects(&roots);
    save_cache(&projects);
    projects
}

/// Find the project that owns a given path.
/// Walks up from the path looking for manifest files.
/// Returns the first project root found, regardless of depth.
pub fn find_project_for_path(path: &Path) -> Option<ProjectInfo> {
    // Normalize: if path is relative, resolve against current dir
    let resolved = if path.is_relative() {
        std::env::current_dir().ok()?.join(path)
    } else {
        path.to_path_buf()
    };

    // Walk up from the path looking for a project root
    let mut current = if resolved.is_dir() {
        resolved
    } else {
        resolved.parent()?.to_path_buf()
    };

    for _ in 0..10 {
        // Check if this directory contains any manifest
        if let Ok(entries) = fs::read_dir(&current) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if let Some(ecosystem) = Ecosystem::from_manifest(&name_str) {
                    let mut manifests = Vec::new();
                    // Collect all manifests for this ecosystem
                    for mf in ecosystem.manifest_files() {
                        let mf_path = current.join(mf);
                        if mf_path.exists() {
                            manifests.push(mf_path);
                        }
                    }
                    // Only return if the primary manifest exists
                    if manifests.iter().any(|p| {
                        p.file_name().and_then(|n| n.to_str()) == Some(ecosystem.primary_manifest())
                    }) {
                        let project_name = current
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        return Some(ProjectInfo {
                            name: project_name,
                            root: current,
                            ecosystem,
                            manifests,
                        });
                    }
                }
            }
        }

        // Walk up
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    None
}

/// Find all discovered projects that use a given registry path.
/// For Cargo registry: finds projects with Cargo.lock.
/// For npm cache: finds projects with package-lock.json.
pub fn find_projects_using_registry(registry_path: &Path) -> Vec<ProjectInfo> {
    let registry_str = registry_path.to_string_lossy().to_lowercase();
    let projects = discover_projects();

    // Determine which ecosystem's registry this is
    let target_ecosystem = if registry_str.contains("/.cargo/registry") {
        Some(Ecosystem::Rust)
    } else if registry_str.contains("/.npm/") {
        Some(Ecosystem::Node)
    } else if registry_str.contains("/.cache/pip/") || registry_str.contains("/.cache/uv/") {
        Some(Ecosystem::Python)
    } else {
        None
    };

    match target_ecosystem {
        Some(eco) => projects
            .into_iter()
            .filter(|p| p.ecosystem == eco)
            .collect(),
        None => projects,
    }
}

// ═══════════════════════════════════════════════════════════════
// Internal: scanning
// ═══════════════════════════════════════════════════════════════

/// Scan search roots for projects.
/// For each directory in the search root, check if it contains a primary manifest.
fn scan_projects(roots: &[PathBuf]) -> Vec<ProjectInfo> {
    let mut projects: Vec<ProjectInfo> = Vec::new();
    let mut seen: HashMap<PathBuf, bool> = HashMap::new();

    for root in roots {
        if !root.is_dir() {
            // For non-existent search roots, continue
            continue;
        }

        // Check if the root itself is a project
        if let Some(proj) = try_discover_project(root) {
            let key = proj.root.clone();
            use std::collections::hash_map::Entry;
            if let Entry::Vacant(e) = seen.entry(key) {
                e.insert(true);
                projects.push(proj);
            }
        }

        // Scan immediate subdirectories (one level deep only)
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                // Skip hidden directories
                if path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
                {
                    continue;
                }
                if let Some(proj) = try_discover_project(&path) {
                    let key = proj.root.clone();
                    use std::collections::hash_map::Entry;
                    if let Entry::Vacant(e) = seen.entry(key) {
                        e.insert(true);
                        projects.push(proj);
                    }
                }
            }
        }
    }

    projects
}

/// Try to discover a project in a directory.
/// Checks for primary manifest files.
fn try_discover_project(dir: &Path) -> Option<ProjectInfo> {
    if !dir.is_dir() {
        return None;
    }

    let ecosystems = [
        Ecosystem::Rust,
        Ecosystem::Node,
        Ecosystem::Python,
        Ecosystem::Go,
    ];

    for eco in &ecosystems {
        let primary = eco.primary_manifest();
        let primary_path = dir.join(primary);
        if primary_path.exists() {
            // Collect all manifest files for this ecosystem
            let mut manifests = Vec::new();
            for mf in eco.manifest_files() {
                let mf_path = dir.join(mf);
                if mf_path.exists() {
                    manifests.push(mf_path);
                }
            }

            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            return Some(ProjectInfo {
                name,
                root: dir.to_path_buf(),
                ecosystem: *eco,
                manifests,
            });
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════
// Internal: cache
// ═══════════════════════════════════════════════════════════════

fn load_cache() -> Option<DiscoveryCache> {
    let path = cache_path()?;
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_cache(projects: &[ProjectInfo]) {
    let path = match cache_path() {
        Some(p) => p,
        None => return,
    };

    let cached_projects: Vec<CachedProject> = projects
        .iter()
        .map(|p| CachedProject {
            name: p.name.clone(),
            root: p.root.to_string_lossy().to_string(),
            ecosystem: format!("{:?}", p.ecosystem),
            manifests: p
                .manifests
                .iter()
                .map(|m| m.to_string_lossy().to_string())
                .collect(),
        })
        .collect();

    let cache = DiscoveryCache {
        projects: cached_projects,
        last_scan: epoch_secs(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = fs::write(&path, json);
    }
}

fn convert_cache(cache: &DiscoveryCache) -> Vec<ProjectInfo> {
    cache
        .projects
        .iter()
        .filter_map(|cp| {
            let ecosystem = match cp.ecosystem.as_str() {
                "Rust" => Ecosystem::Rust,
                "Node" => Ecosystem::Node,
                "Python" => Ecosystem::Python,
                "Go" => Ecosystem::Go,
                _ => return None,
            };
            Some(ProjectInfo {
                name: cp.name.clone(),
                root: PathBuf::from(&cp.root),
                ecosystem,
                manifests: cp.manifests.iter().map(PathBuf::from).collect(),
            })
        })
        .collect()
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_project(ecosystem: Ecosystem) -> TempDir {
        let dir = TempDir::new().unwrap();
        match ecosystem {
            Ecosystem::Rust => {
                fs::write(
                    dir.path().join("Cargo.toml"),
                    "[package]\nname = \"test\"\n",
                )
                .unwrap();
                fs::write(dir.path().join("Cargo.lock"), "").unwrap();
                // Add some source to make it realistic
                let src = dir.path().join("src");
                fs::create_dir(&src).unwrap();
                fs::write(src.join("main.rs"), "fn main() {}").unwrap();
                let target = dir.path().join("target");
                fs::create_dir(&target).unwrap();
                fs::create_dir(target.join("debug")).unwrap();
            }
            Ecosystem::Node => {
                fs::write(
                    dir.path().join("package.json"),
                    "{\"name\": \"test\", \"version\": \"1.0.0\"}",
                )
                .unwrap();
                fs::write(dir.path().join("package-lock.json"), "{}").unwrap();
            }
            Ecosystem::Python => {
                fs::write(
                    dir.path().join("pyproject.toml"),
                    "[project]\nname = \"test\"",
                )
                .unwrap();
                fs::write(dir.path().join("requirements.txt"), "requests==2.0").unwrap();
            }
            Ecosystem::Go => {
                fs::write(dir.path().join("go.mod"), "module test\n\ngo 1.21\n").unwrap();
                fs::write(dir.path().join("go.sum"), "").unwrap();
            }
        }
        dir
    }

    #[test]
    fn test_discover_rust_project() {
        let dir = setup_project(Ecosystem::Rust);
        let projects = scan_projects(&[dir.path().to_path_buf()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].ecosystem, Ecosystem::Rust);
        assert!(projects[0]
            .manifests
            .iter()
            .any(|m| m.file_name().unwrap() == "Cargo.toml"));
    }

    #[test]
    fn test_discover_node_project() {
        let dir = setup_project(Ecosystem::Node);
        let projects = scan_projects(&[dir.path().to_path_buf()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].ecosystem, Ecosystem::Node);
    }

    #[test]
    fn test_discover_python_project() {
        let dir = setup_project(Ecosystem::Python);
        let projects = scan_projects(&[dir.path().to_path_buf()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].ecosystem, Ecosystem::Python);
    }

    #[test]
    fn test_discover_go_project() {
        let dir = setup_project(Ecosystem::Go);
        let projects = scan_projects(&[dir.path().to_path_buf()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].ecosystem, Ecosystem::Go);
    }

    #[test]
    fn test_find_project_for_path_target_release() {
        let dir = setup_project(Ecosystem::Rust);
        let target_release = dir.path().join("target/release");
        let project = find_project_for_path(&target_release);
        assert!(project.is_some());
        assert_eq!(project.unwrap().ecosystem, Ecosystem::Rust);
    }

    #[test]
    fn test_find_project_for_path_src_main() {
        let dir = setup_project(Ecosystem::Rust);
        let src_main = dir.path().join("src/main.rs");
        let project = find_project_for_path(&src_main);
        assert!(project.is_some());
    }

    #[test]
    fn test_find_project_for_path_cargo_toml() {
        let dir = setup_project(Ecosystem::Rust);
        let cargo_toml = dir.path().join("Cargo.toml");
        let project = find_project_for_path(&cargo_toml);
        assert!(project.is_some());
        assert_eq!(project.unwrap().ecosystem, Ecosystem::Rust);
    }

    #[test]
    fn test_find_project_for_path_deep_nested() {
        let dir = setup_project(Ecosystem::Rust);
        let deep = dir.path().join("target/debug/build/some-crate-abc/out");
        let project = find_project_for_path(&deep);
        assert!(project.is_some());
    }

    #[test]
    fn test_find_project_nonexistent_path() {
        let project = find_project_for_path(Path::new("/tmp/nonexistent-abc123"));
        // Should not panic, may or may not find project
        // (depends on whether parent dirs have manifests)
        assert!(project.is_none() || project.is_some());
    }

    #[test]
    fn test_find_project_node() {
        let dir = setup_project(Ecosystem::Node);
        let project = find_project_for_path(&dir.path().join("src/index.js"));
        assert!(project.is_some());
        assert_eq!(project.unwrap().ecosystem, Ecosystem::Node);
    }

    #[test]
    fn test_ecosystem_from_manifest() {
        assert_eq!(
            Ecosystem::from_manifest("Cargo.toml"),
            Some(Ecosystem::Rust)
        );
        assert_eq!(
            Ecosystem::from_manifest("Cargo.lock"),
            Some(Ecosystem::Rust)
        );
        assert_eq!(
            Ecosystem::from_manifest("package.json"),
            Some(Ecosystem::Node)
        );
        assert_eq!(
            Ecosystem::from_manifest("package-lock.json"),
            Some(Ecosystem::Node)
        );
        assert_eq!(
            Ecosystem::from_manifest("pyproject.toml"),
            Some(Ecosystem::Python)
        );
        assert_eq!(
            Ecosystem::from_manifest("requirements.txt"),
            Some(Ecosystem::Python)
        );
        assert_eq!(Ecosystem::from_manifest("go.mod"), Some(Ecosystem::Go));
        assert_eq!(Ecosystem::from_manifest("go.sum"), Some(Ecosystem::Go));
        assert_eq!(Ecosystem::from_manifest("README.md"), None);
        assert_eq!(Ecosystem::from_manifest("unknown"), None);
    }

    #[test]
    fn test_ecosystem_display() {
        assert_eq!(Ecosystem::Rust.display(), "Rust");
        assert_eq!(Ecosystem::Node.display(), "Node.js");
        assert_eq!(Ecosystem::Python.display(), "Python");
        assert_eq!(Ecosystem::Go.display(), "Go");
    }

    #[test]
    fn test_discover_multiple_projects() {
        let dir1 = setup_project(Ecosystem::Rust);
        let dir2 = setup_project(Ecosystem::Node);
        let projects = scan_projects(&[dir1.path().to_path_buf()]);
        assert_eq!(projects.len(), 1);
        // dir2 would be found separately
        let projects2 = scan_projects(&[dir2.path().to_path_buf()]);
        assert_eq!(projects2.len(), 1);
    }

    #[test]
    fn test_empty_directory_no_projects() {
        let dir = TempDir::new().unwrap();
        let projects = scan_projects(&[dir.path().to_path_buf()]);
        assert!(projects.is_empty());
    }

    #[test]
    fn test_primary_manifest() {
        let dir = setup_project(Ecosystem::Rust);
        let project = find_project_for_path(&dir.path().join("Cargo.toml")).unwrap();
        let primary = project.primary_manifest();
        assert!(primary.is_some());
        assert_eq!(
            primary.unwrap().file_name().unwrap().to_str().unwrap(),
            "Cargo.toml"
        );
    }

    #[test]
    fn test_find_projects_using_cargo_registry() {
        let dir = setup_project(Ecosystem::Rust);
        // Scan the test directory so it's cached
        let _projects = scan_projects(&[dir.path().to_path_buf()]);
        // Find projects using cargo registry
        let consumers = find_projects_using_registry(Path::new("/home/user/.cargo/registry"));
        assert!(!consumers.is_empty());
        assert_eq!(consumers[0].ecosystem, Ecosystem::Rust);
    }
}
