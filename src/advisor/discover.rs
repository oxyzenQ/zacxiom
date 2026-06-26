// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — Phase 1: Opportunity Discovery & Phase 5: Action Override.

use crate::discovery::{Ecosystem, ProjectInfo};
use std::path::Path;

/// Known cleanable directory names, by ecosystem.
/// The planner's classifier is the final authority — these are just
/// candidates to CHECK, not automatic inclusions.
pub(crate) fn ecosystem_candidates(ecosystem: Option<Ecosystem>) -> Vec<&'static str> {
    let mut candidates: Vec<&str> = vec![".cache", "tmp", "logs"];

    match ecosystem {
        Some(Ecosystem::Rust) => {
            candidates.extend_from_slice(&["target", "criterion", "coverage"]);
        }
        Some(Ecosystem::Node) => {
            candidates.extend_from_slice(&[
                "node_modules",
                "dist",
                ".next",
                ".turbo",
                ".parcel-cache",
            ]);
        }
        Some(Ecosystem::Python) => {
            candidates.extend_from_slice(&[
                "__pycache__",
                ".pytest_cache",
                ".mypy_cache",
                ".ruff_cache",
                ".tox",
                ".venv",
            ]);
        }
        Some(Ecosystem::Go) => {
            candidates.extend_from_slice(&[]);
        }
        None => {}
    }

    candidates
}

/// Discover existing cleanable candidates inside a directory.
pub(crate) fn discover_candidates(
    root: &Path,
    ecosystem: Option<Ecosystem>,
) -> Vec<std::path::PathBuf> {
    let candidates = ecosystem_candidates(ecosystem);
    let mut found = Vec::new();

    for name in &candidates {
        let candidate = root.join(name);
        if candidate.exists() {
            found.push(candidate);
        }
    }

    found
}

/// Minimum size (bytes) to be worth showing as a cleanup opportunity.
pub(crate) const MINIMUM_MEANINGFUL_SIZE: u64 = 1_048_576; // 1 MB

/// Detect the Node.js package manager from lockfiles in the project.
pub(crate) fn detect_node_pm(project: &ProjectInfo) -> &'static str {
    for m in &project.manifests {
        let name = m.file_name().and_then(|n| n.to_str()).unwrap_or("");
        match name {
            "pnpm-lock.yaml" => return "pnpm",
            "yarn.lock" => return "yarn",
            "package-lock.json" => return "npm",
            _ => {}
        }
    }
    "npm" // Default fallback
}

/// Override the planner's action with an ecosystem-aware command
/// when the planner falls through to generic wording.
pub(crate) fn ecosystem_action_override(
    display_name: &str,
    ecosystem: Option<Ecosystem>,
    project: Option<&ProjectInfo>,
    planner_action: &str,
) -> (String, bool) {
    let is_generic = planner_action == "Remove temporary files."
        || planner_action == "Clear application cache."
        || planner_action == "Manual cleanup"
        || planner_action.is_empty();

    if !is_generic {
        return (planner_action.to_string(), false);
    }

    let name = display_name.trim_end_matches('/');

    match ecosystem {
        Some(Ecosystem::Node) => {
            let pm = project.map_or("npm", detect_node_pm);
            match name {
                "node_modules" => (format!("{pm} install"), true),
                "dist" => (format!("{pm} run build"), true),
                ".next" => ("next build".to_string(), true),
                ".turbo" => ("turbo build".to_string(), true),
                ".parcel-cache" => (format!("{pm} run build"), true),
                _ => (planner_action.to_string(), false),
            }
        }
        Some(Ecosystem::Rust) => match name {
            "target" => ("cargo clean".to_string(), true),
            "criterion" => ("cargo clean".to_string(), true),
            "coverage" => ("cargo clean".to_string(), true),
            _ => (planner_action.to_string(), false),
        },
        Some(Ecosystem::Python) => match name {
            "__pycache__" => (
                "find . -type d -name __pycache__ -exec rm -rf {} +".to_string(),
                true,
            ),
            ".pytest_cache" => ("pytest --cache-clear".to_string(), true),
            ".mypy_cache" => ("rm -rf .mypy_cache".to_string(), true),
            ".ruff_cache" => ("ruff clean".to_string(), true),
            ".venv" => ("python -m venv .venv && pip install -e .".to_string(), true),
            _ => (planner_action.to_string(), false),
        },
        Some(Ecosystem::Go) => (planner_action.to_string(), false),
        None => (planner_action.to_string(), false),
    }
}
