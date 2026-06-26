// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Workspace Intelligence — v8.6
//!
//! Multi-project discovery, workspace summary, and cross-project cleanup planning.
//!
//! A "workspace" is a directory containing one or more projects.
//! This module discovers all projects within a workspace and provides
//! summary information and aggregated cleanup recommendations.

use crate::discovery::{self, Ecosystem, ProjectInfo};
use crate::planner;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A discovered project within a workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceProject {
    /// Project root directory.
    pub path: PathBuf,
    /// Project name.
    pub name: String,
    /// Detected ecosystem.
    pub ecosystem: Option<Ecosystem>,
    /// Estimated reclaimable bytes.
    pub reclaimable: u64,
    /// Number of cleanable artifacts found.
    pub artifact_count: usize,
}

/// Workspace-wide summary.
#[derive(Debug, Clone)]
pub struct WorkspaceSummary {
    /// Root directory analyzed.
    pub root: PathBuf,
    /// All discovered projects.
    pub projects: Vec<WorkspaceProject>,
    /// Projects grouped by ecosystem.
    pub by_ecosystem: HashMap<String, (usize, u64)>,
    /// Total potential reclaim across all projects.
    pub total_reclaimable: u64,
    /// Total number of projects found.
    pub project_count: usize,
}

/// Discover all projects within a workspace directory.
///
/// Walks the directory tree (depth-limited) to find project markers
/// (Cargo.toml, package.json, etc.) and groups them into projects.
///
/// Max depth: 3 levels to avoid scanning entire filesystem.
pub fn discover_workspace(root: &Path) -> WorkspaceSummary {
    let mut projects: Vec<WorkspaceProject> = Vec::new();
    let mut seen: HashMap<PathBuf, bool> = HashMap::new();

    discover_recursive(root, 0, 3, &mut projects, &mut seen);

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

/// Recursive project discovery with depth limit.
fn discover_recursive(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    projects: &mut Vec<WorkspaceProject>,
    seen: &mut HashMap<PathBuf, bool>,
) {
    if depth > max_depth || !dir.is_dir() {
        return;
    }

    // Check if this directory itself is a project root
    if let Ok(canonical) = dir.canonicalize() {
        if seen.contains_key(&canonical) {
            return;
        }
        seen.insert(canonical, true);
    }

    let project_info = discovery::find_project_for_path(dir);

    if let Some(project) = project_info {
        // This is a project root — compute reclaimable estimates
        let reclaimable = estimate_project_reclaim(dir, &project);
        let artifact_count = count_cleanable_artifacts(dir, &project);

        projects.push(WorkspaceProject {
            path: dir.to_path_buf(),
            name: project.name,
            ecosystem: Some(project.ecosystem),
            reclaimable,
            artifact_count,
        });
    }

    // Continue scanning children (depth-limited)
    if depth < max_depth {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let child = entry.path();
                if child.is_dir() {
                    // Skip hidden directories and well-known non-project dirs
                    let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with('.') && name != ".config" {
                        continue;
                    }
                    if matches!(
                        name,
                        "node_modules"
                            | "target"
                            | "__pycache__"
                            | "vendor"
                            | "dist"
                            | "build"
                            | ".git"
                    ) {
                        continue;
                    }
                    discover_recursive(&child, depth + 1, max_depth, projects, seen);
                }
            }
        }
    }
}

/// Estimate reclaimable bytes for a project by checking known artifact directories.
fn estimate_project_reclaim(root: &Path, project: &ProjectInfo) -> u64 {
    let mut total: u64 = 0;

    // Common cleanable subdirectories
    let candidates = match project.ecosystem {
        Ecosystem::Rust => vec!["target"],
        Ecosystem::Node => vec!["node_modules", "dist", ".next", ".nuxt", "coverage"],
        Ecosystem::Python => vec!["__pycache__", ".venv", "venv", "dist", ".tox"],
        Ecosystem::Go => vec!["vendor"],
    };

    for candidate in &candidates {
        let path = root.join(candidate);
        if path.is_dir() {
            // Use planner to estimate size — reuses caching from v8.6
            let plan = planner::plan(&path);
            total += plan.estimated_reclaimable_bytes;
        }
    }

    total
}

/// Count the number of cleanable artifact directories found.
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

    // Grouped by ecosystem
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_workspace_empty() {
        let dir = TempDir::new().unwrap();
        let summary = discover_workspace(dir.path());
        assert_eq!(summary.project_count, 0);
    }

    #[test]
    fn test_discover_workspace_single_rust_project() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/binary"), vec![0u8; 1024]).unwrap();

        let summary = discover_workspace(root);
        assert!(
            summary.project_count >= 1,
            "Expected at least 1 project, got {}",
            summary.project_count
        );
        assert!(summary.total_reclaimable > 0);
    }

    #[test]
    fn test_discover_workspace_multiple_projects() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Rust project
        let rust = root.join("rust-project");
        fs::create_dir(&rust).unwrap();
        fs::write(rust.join("Cargo.toml"), "[package]\nname = \"rust\"\n").unwrap();

        // Node project
        let node = root.join("node-project");
        fs::create_dir(&node).unwrap();
        fs::write(
            node.join("package.json"),
            "{\"name\": \"node\", \"version\": \"1.0\"}",
        )
        .unwrap();

        let summary = discover_workspace(root);
        assert!(
            summary.project_count >= 2,
            "Expected at least 2 projects, got {}",
            summary.project_count
        );
        assert!(summary.by_ecosystem.contains_key("Rust"));
        assert!(summary.by_ecosystem.contains_key("Node.js"));
    }

    #[test]
    fn test_render_workspace_summary() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();

        let summary = discover_workspace(root);
        let output = render_workspace_summary(&summary);
        assert!(output.contains("WORKSPACE SUMMARY"));
        assert!(output.contains("Rust:"));
    }
}
