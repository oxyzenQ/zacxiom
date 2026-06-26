// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Workspace Intelligence — v8.7
//!
//! Multi-project discovery, workspace summary, and cross-project cleanup planning.
//!
//! v8.7.1: Fixed false-positive subdirectory detection.
//! Uses `is_project_root` (no parent traversal) instead of `find_project_for_path`.

use crate::discovery::{self, Ecosystem, ProjectInfo};
use crate::planner;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Subdirectories that are NEVER projects.
const NON_PROJECT_DIRS: &[&str] = &[
    "src",
    "docs",
    "assets",
    "scripts",
    "tests",
    "examples",
    "benches",
    "target",
    "node_modules",
    "__pycache__",
    "vendor",
    "dist",
    "build",
    ".git",
    ".github",
    ".vscode",
    ".idea",
    "out",
    "bin",
    "obj",
    "include",
    "lib",
    "tmp",
    "temp",
];

/// A discovered project within a workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceProject {
    pub path: PathBuf,
    pub name: String,
    pub ecosystem: Option<Ecosystem>,
    pub reclaimable: u64,
    pub artifact_count: usize,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSummary {
    pub root: PathBuf,
    pub projects: Vec<WorkspaceProject>,
    pub by_ecosystem: HashMap<String, (usize, u64)>,
    pub total_reclaimable: u64,
    pub project_count: usize,
}

/// Discover all projects within a workspace directory.
///
/// Walk: checks root first → if project found, stop recursing into its children.
/// Only recurses into child directories that are NOT themselves inside a known
/// project root and that could contain nested projects.
pub fn discover_workspace(root: &Path) -> WorkspaceSummary {
    let mut projects: Vec<WorkspaceProject> = Vec::new();

    // Step 1: Check if root itself is a project
    if let Some(info) = discovery::is_project_root(root) {
        add_project(&mut projects, root, &info);
        // Root IS a project — only check for NESTED projects in immediate children
        discover_nested(root, &mut projects);
    } else {
        // Root is NOT a project — scan children for projects
        discover_children(root, &mut projects, &mut HashSet::new());
    }

    let mut by_ecosystem: HashMap<String, (usize, u64)> = HashMap::new();
    let mut total_reclaimable: u64 = 0;

    for proj in &projects {
        let eco_name = proj
            .ecosystem
            .map(|e| e.display().to_string())
            .unwrap_or_else(|| "Other".to_string());
        let entry = by_ecosystem.entry(eco_name).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += proj.reclaimable;
        total_reclaimable += proj.reclaimable;
    }

    WorkspaceSummary {
        root: root.to_path_buf(),
        project_count: projects.len(),
        projects,
        by_ecosystem,
        total_reclaimable,
    }
}

/// Add a project to the workspace list.
fn add_project(projects: &mut Vec<WorkspaceProject>, root: &Path, info: &ProjectInfo) {
    let reclaimable = estimate_project_reclaim(root, info);
    let artifact_count = count_cleanable_artifacts(root, info);

    projects.push(WorkspaceProject {
        path: root.to_path_buf(),
        name: info.name.clone(),
        ecosystem: Some(info.ecosystem),
        reclaimable,
        artifact_count,
    });
}

/// Discover nested projects inside a known project root.
///
/// Only checks IMMEDIATE children. Does NOT recurse into subdirectories
/// that are standard non-project dirs (src/, docs/, targets/, etc.).
fn discover_nested(root: &Path, projects: &mut Vec<WorkspaceProject>) {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if NON_PROJECT_DIRS.contains(&name) {
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            // Check if this child is itself a project (nested)
            if let Some(info) = discovery::is_project_root(&child) {
                add_project(projects, &child, &info);
            }
        }
    }
}

/// Discover projects by scanning children (when root is not a project).
fn discover_children(
    dir: &Path,
    projects: &mut Vec<WorkspaceProject>,
    seen: &mut HashSet<PathBuf>,
) {
    if let Ok(canonical) = dir.canonicalize() {
        if seen.contains(&canonical) {
            return;
        }
        seen.insert(canonical);
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if NON_PROJECT_DIRS.contains(&name) {
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            if let Some(info) = discovery::is_project_root(&child) {
                add_project(projects, &child, &info);
            } else {
                // Not a project — recurse further
                discover_children(&child, projects, seen);
            }
        }
    }
}

/// Estimate reclaimable bytes for a project by checking known artifact directories.
fn estimate_project_reclaim(root: &Path, project: &ProjectInfo) -> u64 {
    let mut total: u64 = 0;

    let candidates = match project.ecosystem {
        Ecosystem::Rust => vec!["target"],
        Ecosystem::Node => vec!["node_modules", "dist", ".next", ".nuxt", "coverage"],
        Ecosystem::Python => vec!["__pycache__", ".venv", "venv", "dist", ".tox"],
        Ecosystem::Go => vec!["vendor"],
    };

    for candidate in &candidates {
        let path = root.join(candidate);
        if path.is_dir() {
            let plan = planner::plan(&path);
            total += plan.estimated_reclaimable_bytes;
        }
    }

    total
}

fn count_cleanable_artifacts(root: &Path, project: &ProjectInfo) -> usize {
    let candidates = match project.ecosystem {
        Ecosystem::Rust => vec!["target"],
        Ecosystem::Node => vec!["node_modules", "dist", ".next", ".nuxt", "coverage"],
        Ecosystem::Python => vec!["__pycache__", ".venv", "venv", "dist", ".tox"],
        Ecosystem::Go => vec!["vendor"],
    };

    candidates.iter().filter(|c| root.join(c).exists()).count()
}

/// Format a workspace summary as human-readable text.
pub fn render_workspace_summary(summary: &WorkspaceSummary) -> String {
    use crate::display::human_size;

    let mut out = String::new();
    out.push_str(&crate::color::section_header("WORKSPACE SUMMARY"));
    out.push_str(&format!(
        "  Root:              {}\n",
        summary.root.display()
    ));
    out.push_str(&format!("  Projects found:    {}\n", summary.project_count));
    out.push_str(&format!(
        "  Total reclaimable: {}\n",
        human_size(summary.total_reclaimable)
    ));
    out.push('\n');

    if summary.project_count == 0 {
        out.push_str("  No projects detected in this workspace.\n");
        out.push_str("  Try running `zacxiom plan` on individual project directories.\n");
        return out;
    }

    let mut ecosystems: Vec<_> = summary.by_ecosystem.iter().collect();
    ecosystems.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));

    for (eco, (count, reclaim)) in &ecosystems {
        out.push_str(&format!(
            "  {:<20} {:>3} projects  {:>10}\n",
            format!("{}:", eco),
            count,
            human_size(*reclaim)
        ));
    }

    out.push('\n');
    out.push_str("  Projects:\n");
    for proj in &summary.projects {
        let eco_display = proj
            .ecosystem
            .map(|e| e.display().to_string())
            .unwrap_or_else(|| "?".to_string());
        out.push_str(&format!(
            "    {:<30} {:<10} {:>10}  ({} artifacts)\n",
            truncate_path(&proj.path, 30),
            eco_display,
            human_size(proj.reclaimable),
            proj.artifact_count,
        ));
    }

    out
}

fn truncate_path(path: &Path, max_len: usize) -> String {
    let s = path.to_string_lossy();
    if s.len() <= max_len {
        return s.to_string();
    }
    format!("...{}", &s[s.len().saturating_sub(max_len - 3)..])
}

/// Generate cross-project cleanup recommendations.
pub fn cross_project_recommendations(summary: &WorkspaceSummary) -> Vec<(String, Vec<String>)> {
    let mut recs: Vec<(String, Vec<String>)> = Vec::new();

    let mut seen_names: HashSet<String> = HashSet::new();
    let mut unique_projects: Vec<String> = Vec::new();

    for proj in &summary.projects {
        if seen_names.insert(proj.name.clone()) {
            unique_projects.push(proj.name.clone());
        }
    }

    for (eco, (count, reclaim)) in &summary.by_ecosystem {
        if *count > 1 {
            let eco_projects: Vec<String> = summary
                .projects
                .iter()
                .filter(|p| {
                    p.ecosystem.map(|e| e.display().to_string()).as_deref() == Some(eco.as_str())
                })
                .map(|p| p.name.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let recommendation = format!(
                "{count} {eco} projects found — consolidated potential reclaim: {}",
                crate::display::human_size(*reclaim),
            );
            recs.push((recommendation, eco_projects));
        } else if *count == 1 {
            let recommendation = format!("1 {eco} project found",);
            let eco_projects: Vec<String> = summary
                .projects
                .iter()
                .filter(|p| {
                    p.ecosystem.map(|e| e.display().to_string()).as_deref() == Some(eco.as_str())
                })
                .map(|p| p.name.clone())
                .collect();
            recs.push((recommendation, eco_projects));
        }
    }

    recs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ═══════════════════════════════════════════════════════════
    // Regression tests — v8.7.1: subdirectory false positives
    // ═══════════════════════════════════════════════════════════

    fn make_rust_project(dir: &Path, name: &str) {
        fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\n"),
        )
        .unwrap();
        fs::create_dir(dir.join("src")).unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    }

    fn make_node_project(dir: &Path, name: &str) {
        fs::write(
            dir.join("package.json"),
            format!("{{\"name\": \"{name}\", \"version\": \"1.0\"}}"),
        )
        .unwrap();
    }

    #[test]
    fn test_single_rust_project_not_many() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        make_rust_project(root, "myproject");
        fs::create_dir(root.join("docs")).unwrap();
        fs::create_dir(root.join("scripts")).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();

        let summary = discover_workspace(root);
        assert_eq!(
            summary.project_count,
            1,
            "Expected 1 project, got {}: {:?}",
            summary.project_count,
            summary.projects.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_subdirs_not_projects() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        make_rust_project(root, "app");
        fs::create_dir(root.join("docs")).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        fs::create_dir(root.join("scripts")).unwrap();

        let summary = discover_workspace(root);
        let names: Vec<&str> = summary.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(!names.contains(&"src"), "src/ should not be a project");
        assert!(!names.contains(&"docs"), "docs/ should not be a project");
        assert!(
            !names.contains(&"assets"),
            "assets/ should not be a project"
        );
        assert!(
            !names.contains(&"scripts"),
            "scripts/ should not be a project"
        );
        assert_eq!(summary.project_count, 1);
    }

    #[test]
    fn test_target_not_a_project() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        make_rust_project(root, "app");
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/binary"), vec![0u8; 1024]).unwrap();

        let summary = discover_workspace(root);
        let names: Vec<&str> = summary.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(
            !names.contains(&"target"),
            "target/ should not be a project"
        );
        assert!(!names.contains(&"debug"), "debug/ should not be a project");
        assert_eq!(summary.project_count, 1);
    }

    #[test]
    fn test_nested_rust_project() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let parent_dir = root.join("parent");
        fs::create_dir(&parent_dir).unwrap();
        make_rust_project(&parent_dir, "parent");
        let tools_dir = parent_dir.join("tools");
        fs::create_dir(&tools_dir).unwrap();
        make_rust_project(&tools_dir, "tools");

        let summary = discover_workspace(&parent_dir);
        assert_eq!(
            summary.project_count, 2,
            "Should find 2 projects (parent + nested tools)"
        );
        let names: Vec<&str> = summary.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"parent"));
        assert!(names.contains(&"tools"));
    }

    #[test]
    fn test_node_project_single() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        make_node_project(root, "nodeapp");
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::create_dir(root.join("dist")).unwrap();

        let summary = discover_workspace(root);
        assert_eq!(summary.project_count, 1);
        assert!(!summary.projects.iter().any(|p| p.name == "node_modules"));
        assert!(!summary.projects.iter().any(|p| p.name == "dist"));
    }

    #[test]
    fn test_mixed_rust_node_workspace() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let rust_dir = root.join("rust-app");
        fs::create_dir(&rust_dir).unwrap();
        make_rust_project(&rust_dir, "rust-app");
        let node_dir = root.join("node-app");
        fs::create_dir(&node_dir).unwrap();
        make_node_project(&node_dir, "node-app");

        let summary = discover_workspace(root);
        assert_eq!(summary.project_count, 2);
        assert!(summary.by_ecosystem.contains_key("Rust"));
        assert!(summary.by_ecosystem.contains_key("Node.js"));
    }

    #[test]
    fn test_empty_dir() {
        let dir = TempDir::new().unwrap();
        let summary = discover_workspace(dir.path());
        assert_eq!(summary.project_count, 0);
    }

    #[test]
    fn test_no_duplicate_project_names() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        make_rust_project(root, "zacxiom");

        let summary = discover_workspace(root);
        let names: HashSet<&str> = summary.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names.len(),
            summary.project_count,
            "All project names must be unique"
        );
    }

    #[test]
    fn test_render_workspace_summary() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        make_rust_project(root, "test");

        let summary = discover_workspace(root);
        let output = render_workspace_summary(&summary);
        assert!(output.contains("WORKSPACE SUMMARY"));
        assert!(output.contains("Rust:"));
    }
}
