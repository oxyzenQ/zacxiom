// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dependency Awareness Engine — artifact lifecycle graph.
//!
//! v7.1: Builds a directed graph of artifacts to answer:
//!   - Who created this artifact?
//!   - Who consumes (depends on) this artifact?
//!   - What is the regeneration path?
//!   - What is the deletion blast radius?
//!
//! The dependency graph powers the explain command's INTELLIGENCE section
//! and enables future features like dependency-aware cleanup ordering.

use crate::engine::{Category, ClassificationResult};
use std::collections::HashMap;
use std::path::PathBuf;

/// A node in the dependency graph — one artifact.
#[derive(Debug, Clone)]
pub struct ArtifactNode {
    /// The artifact path.
    pub path: PathBuf,
    /// Artifact category.
    pub category: Category,
    /// Who/what created this artifact.
    pub created_by: String,
    /// How to regenerate (command or process).
    pub regenerated_by: String,
    /// What this artifact depends on (logical dependencies, not filesystem).
    pub depends_on: Vec<String>,
    /// What are the consumers of this artifact?
    pub consumed_by: Vec<String>,
    /// What breaks if this artifact is deleted?
    pub deletion_impact: String,
    /// Is this a root node (not depending on anything local)?
    pub is_root: bool,
    /// Is this a leaf node (nothing depends on it)?
    pub is_leaf: bool,
}

/// Directed dependency graph between artifacts.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// nodes keyed by canonical path.
    nodes: HashMap<String, ArtifactNode>,
    /// edges: (consumer, producer) pairs.
    edges: Vec<(String, String)>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an artifact node to the graph.
    pub fn add_node(&mut self, node: ArtifactNode) {
        let key = node.path.to_string_lossy().to_string();
        self.nodes.insert(key, node);
    }

    /// Add a dependency edge: consumer → depends on → producer.
    pub fn add_edge(&mut self, consumer: &str, producer: &str) {
        self.edges
            .push((consumer.to_string(), producer.to_string()));
        // Update consumer node's consumed_by for the producer
        if let Some(node) = self.nodes.get_mut(producer) {
            if !node.consumed_by.contains(&consumer.to_string()) {
                node.consumed_by.push(consumer.to_string());
            }
        }
    }

    /// Find all artifacts that consume the given path.
    pub fn consumers_of(&self, path: &str) -> Vec<&ArtifactNode> {
        self.edges
            .iter()
            .filter(|(consumer, _)| consumer == path)
            .filter_map(|(_, producer)| self.nodes.get(producer))
            .collect()
    }

    /// Find all artifacts that the given path depends on.
    pub fn dependencies_of(&self, path: &str) -> Vec<&ArtifactNode> {
        self.edges
            .iter()
            .filter(|(_, producer)| producer == path)
            .filter_map(|(consumer, _)| self.nodes.get(consumer))
            .collect()
    }

    /// Get a node by path.
    pub fn get(&self, path: &str) -> Option<&ArtifactNode> {
        self.nodes.get(path)
    }

    /// Number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Is the graph empty?
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Build a dependency graph from classification results.
/// Analyzes parent-child relationships and known artifact patterns
/// to construct the lifecycle graph.
pub fn build_graph(results: &[ClassificationResult]) -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    for r in results {
        let _key = r.path.to_string_lossy().to_string();
        let node = ArtifactNode {
            path: r.path.clone(),
            category: r.category,
            created_by: r.created_by.clone(),
            regenerated_by: r.regenerated_by.clone(),
            depends_on: parse_depends_list(&r.depends_on),
            consumed_by: Vec::new(),
            deletion_impact: r.deletion_impact.clone(),
            is_root: r.depends_on.is_empty(),
            is_leaf: true, // will be updated by edges
        };
        graph.add_node(node);
    }

    // Build edges from depends_on relationships
    // Collect edge candidates first to avoid borrow issues
    let mut edge_candidates: Vec<(String, String)> = Vec::new();
    for r in results {
        let consumer = r.path.to_string_lossy().to_string();
        let deps = parse_depends_list(&r.depends_on);
        for dep in &deps {
            // Find the producer node that matches this dependency
            for key in graph.nodes.keys() {
                if key.ends_with(dep) || dep.contains(key.as_str()) {
                    edge_candidates.push((consumer.clone(), key.clone()));
                    break;
                }
            }
            // If depends_on references a known sibling, link it
            if let Some(parent) = r.path.parent() {
                for dep in &deps {
                    let sibling = parent.join(dep);
                    let sibling_key = sibling.to_string_lossy().to_string();
                    if graph.nodes.contains_key(&sibling_key) {
                        edge_candidates.push((consumer.clone(), sibling_key));
                    }
                }
            }
        }
    }

    // Apply edges after iteration
    for (consumer, producer) in edge_candidates {
        graph.add_edge(&consumer, &producer);
    }

    graph
}

/// Parse a comma-separated or newline-separated dependency list.
fn parse_depends_list(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(&[',', '\n'][..])
        .map(|part| part.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Generate classification reasoning for an artifact.
/// Explains WHY the classifier chose a specific category.
/// This is different from the `reasons` field — `reasons` explains
/// what matched, while `classification_reasoning` explains the
/// semantic reasoning behind the classification.
pub fn generate_reasoning(result: &ClassificationResult) -> Vec<String> {
    let mut reasoning = Vec::new();

    let cat = &result.category;

    // Why is this classified as [category]?
    match cat {
        Category::DependencySource | Category::DownloadedArtifact => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push(
                "  - Contains downloaded source/package content (not user-authored)".to_string(),
            );
            reasoning
                .push("  - Consumed by build tools during compilation/installation".to_string());
            reasoning.push("  - Expensive to re-download (time + bandwidth)".to_string());
            reasoning.push("  - Not user-authored content".into());
            reasoning.push(
                "Why not Cache? — required as source for builds, survives across sessions"
                    .to_string(),
            );
            reasoning.push(
                "Why not BuildArtifact? — downloaded from network, not generated locally"
                    .to_string(),
            );
        }
        Category::BuildCache => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Generated from source code by a build command".into());
            reasoning.push("  - Fully regenerable without network access".into());
            reasoning.push("  - No user-authored content — purely derived output".into());
            reasoning.push(
                "Why not Cache? — code-derived, not runtime-generated; requires build step".into(),
            );
            reasoning.push(
                "Why not SourceDirectory? — contains compiled artifacts, not source code".into(),
            );
        }
        Category::Cache | Category::CacheRegistry => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Contains runtime-generated temporary data".into());
            reasoning.push("  - Applications rebuild automatically when needed".into());
            reasoning.push("  - No user data or configuration stored here".into());
            reasoning.push("Why not ApplicationData? — disposable, auto-regenerated".into());
        }
        Category::BrowserCache => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning
                .push("  - Contains rendered pages, assets, and temporary internet files".into());
            reasoning.push("  - Browser rebuilds automatically while browsing".into());
            reasoning.push("  - No bookmarks, passwords, or settings affected".into());
            reasoning.push("Why not ApplicationData? — zero user state, fully transient".into());
        }
        Category::TemporaryFile => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Located in /tmp or system temp directory".into());
            reasoning.push("  - Created for short-lived use, designed to be cleaned".into());
            reasoning.push("  - No persistent state or configuration".into());
        }
        Category::ToolchainManager | Category::ToolchainInstallation => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Installed development tooling, not disposable cache".into());
            reasoning
                .push("  - Required for building software — deleting breaks development".into());
            reasoning
                .push("  - Regenerable but expensive (gigabytes, minutes to reinstall)".into());
            reasoning.push(
                "Why not Cache? — installed software, survives sessions, manually managed".into(),
            );
            reasoning.push("Why not SystemBinary? — user-space tooling, not OS-critical".into());
        }
        Category::InstalledSoftware => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - User-installed package or application".into());
            reasoning.push("  - Not auto-regenerated — must be explicitly reinstalled".into());
            reasoning.push("Why not Cache? — manually installed, not auto-regenerated".into());
        }
        Category::ProjectWorkspace | Category::SourceDirectory => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Contains user-authored source code".into());
            reasoning.push("  - Irreplaceable without version control restoration".into());
            reasoning.push("  - Not derived, not generated, not disposable".into());
        }
        Category::BuildManifest => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Defines project identity and build configuration".into());
            reasoning.push("  - Without it, build system cannot function".into());
            reasoning.push(
                "Why not ApplicationConfiguration? — project-defining, not generic settings".into(),
            );
        }
        Category::DependencyLockfile => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Pins exact dependency versions for reproducible builds".into());
            reasoning.push("  - Auto-generated from manifest + dependency resolution".into());
            reasoning.push(
                "Why not BuildArtifact? — version-pinning metadata, not compiled output".into(),
            );
        }
        Category::SecurityCredential => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Cryptographic identity file".into());
            reasoning
                .push("  - Cannot be regenerated — creates new keys, old access is lost".into());
            reasoning.push("  - Permanent access loss if deleted".into());
        }
        Category::SystemBinary | Category::SystemConfiguration | Category::SystemData => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Part of the operating system or installed packages".into());
            reasoning.push("  - Required for system operation or application functionality".into());
            reasoning.push("  - Not regenerable without package reinstall or OS repair".into());
        }
        Category::ApplicationData => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - Contains user-generated or application-saved data".into());
            reasoning.push("  - May include databases, saved states, sync data".into());
            reasoning.push("  - Some data may be cloud-synced, but not guaranteed".into());
            reasoning.push("Why not Cache? — persistent user state, not auto-regenerated".into());
        }
        Category::Unknown => {
            reasoning.push(format!("Why {}?", cat.display()));
            reasoning.push("  - No classification rule matched this path".into());
            reasoning.push("  - May be a user-created directory with no known pattern".into());
            reasoning.push("  - Manual review recommended".into());
        }
        _ => {
            // For categories without specific reasoning, give a generic one
            if !result.reasons.is_empty() {
                reasoning.push(format!("Why {}?", cat.display()));
                for r in &result.reasons {
                    reasoning.push(format!("  - {r}"));
                }
            }
        }
    }

    reasoning
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Category;
    use std::path::Path;

    #[test]
    fn test_parse_depends_list() {
        assert_eq!(parse_depends_list(""), Vec::<String>::new());
        assert_eq!(
            parse_depends_list("Cargo.toml"),
            vec!["Cargo.toml".to_string()]
        );
        assert_eq!(
            parse_depends_list("Cargo.toml, Cargo.lock"),
            vec!["Cargo.toml".to_string(), "Cargo.lock".to_string()]
        );
        assert_eq!(
            parse_depends_list("Cargo.toml\nCargo.lock\nsrc/"),
            vec![
                "Cargo.toml".to_string(),
                "Cargo.lock".to_string(),
                "src/".to_string()
            ]
        );
    }

    #[test]
    fn test_empty_graph() {
        let graph = DependencyGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = DependencyGraph::new();
        let node = ArtifactNode {
            path: Path::new("/home/user/.cargo/registry").to_path_buf(),
            category: Category::DownloadedArtifact,
            created_by: "Cargo".into(),
            regenerated_by: "cargo fetch".into(),
            depends_on: vec!["Cargo.toml".into(), "crates.io".into()],
            consumed_by: vec![],
            deletion_impact: "Next build re-downloads all crates".into(),
            is_root: false,
            is_leaf: true,
        };
        graph.add_node(node);
        assert_eq!(graph.len(), 1);
        assert!(!graph.is_empty());
    }

    #[test]
    fn test_generate_reasoning_for_build_cache() {
        let mut result =
            ClassificationResult::new(Path::new("/home/user/project/target").to_path_buf());
        result.category = Category::BuildCache;
        let reasoning = generate_reasoning(&result);
        assert!(!reasoning.is_empty());
        assert!(reasoning.iter().any(|r| r.contains("Build Output")));
        assert!(reasoning.iter().any(|r| r.contains("compiled artifacts")));
    }

    #[test]
    fn test_generate_reasoning_for_dependency_source() {
        let mut result =
            ClassificationResult::new(Path::new("/home/user/.cargo/registry").to_path_buf());
        result.category = Category::DownloadedArtifact;
        let reasoning = generate_reasoning(&result);
        assert!(!reasoning.is_empty());
        assert!(reasoning.iter().any(|r| r.contains("Downloaded")));
        assert!(reasoning.iter().any(|r| r.contains("Cache")));
    }
}
