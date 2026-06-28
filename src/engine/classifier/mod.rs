// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Multi-layer scoring engine — combines evidence from all layers.

use super::metadata;
use super::types::{Category, ClassificationResult, RiskLevel};
use std::path::Path;

/// Fast classification without confidence scoring (v6.3.1).
/// Returns (category_display_string, confidence).
/// v7.2: Supports parent-child inheritance for scan pipeline consistency.
/// v11: Active environment protection — checks if path is in an active SDK/toolchain.
pub fn classify_fast(path: &Path) -> (&'static str, u8) {
    // v11: Active environment protection — highest priority check
    if crate::environment::is_active_environment(path).is_some() {
        return (Category::ProtectedActiveEnvironment.display(), 100);
    }

    let lower = path.to_string_lossy().to_lowercase();
    let rules = super::rules::rule_database();
    for rule in rules {
        if (rule.matches)(path, &lower) {
            return (rule.category.display(), 100);
        }
    }
    // v7.2: Parent-child inheritance for scan pipeline
    // Walk up parents to find a classified ancestor
    let mut ancestor = match path.parent() {
        Some(p) => p,
        None => return (Category::Unknown.display(), 0),
    };
    for _ in 0..5 {
        let anc_lower = ancestor.to_string_lossy().to_lowercase();
        for rule in rules {
            if (rule.matches)(ancestor, &anc_lower) {
                return (rule.category.display(), 60);
            }
        }
        match ancestor.parent() {
            Some(p) => ancestor = p,
            None => break,
        }
    }
    (Category::Unknown.display(), 0)
}

/// Classify a path using the full rule engine + metadata analysis.
///
/// v7.2 Context Inheritance Engine:
///   Layer 1: Rule database
///   Layer 1.5: Project override — project roots in Desktop/Documents/etc.
///              override location-based rules (fixes Desktop/labs-coding/...)
///   Layer 2.5: Project/workspace detection (filesystem-aware)
///   Layer 3.5: Parent-child inheritance — children of classified parents
///              inherit their parent's category (fixes target/debug, target/release)
pub fn classify(path: &Path) -> ClassificationResult {
    let path_str = path.to_string_lossy();
    let lower = path_str.to_lowercase();

    let mut result = ClassificationResult::new(path.to_path_buf());

    // Size if available
    result.size = metadata::file_size(path);

    // ── Layer 1: Rule database (structured path matching) ─────
    let rules = super::rules::rule_database(); // cached OnceLock
    let mut matched = false;
    // Track if we matched a location-based rule (Desktop, Documents, etc.)
    // that should be overridden if project markers are found.
    let mut is_location_overrideable = false;

    for rule in rules {
        if (rule.matches)(path, &lower) {
            result.category = rule.category;
            result.risk_level = rule.risk_level;
            result.regenerable = rule.regenerable;
            result.matched_by = rule.name.to_string();
            result.reasons.push(rule.reason.to_string());
            // v7: Propagate artifact intelligence fields
            result.created_by = rule.created_by.to_string();
            result.regenerated_by = rule.regenerated_by.to_string();
            result.depends_on = rule.depends_on.to_string();
            result.deletion_impact = rule.deletion_impact.to_string();
            matched = true;

            // Mark location-based rules for potential project override
            // v8.3.1: Also mark TemporaryFile — project signals must outrank temp-location.
            is_location_overrideable = matches!(
                rule.category,
                Category::UserDesktop
                    | Category::UserDocument
                    | Category::UserMedia
                    | Category::UserHomeRoot
                    | Category::TemporaryFile
            );
            break;
        }
    }

    // ── Layer 1.5: Project override — detect projects in Desktop/Documents/etc. ──
    // Fixes ~/Desktop/labs-coding/cosmostrix classified as User Desktop
    // when it's actually a project workspace with Cargo.toml / package.json / .git
    if is_location_overrideable && path.is_dir() && project_markers_found(path) {
        result.category = Category::ProjectWorkspace;
        result.risk_level = RiskLevel::High;
        result.regenerable = false;
        result.matched_by = "project-override".to_string();
        result.reasons.clear();
        result
            .reasons
            .push("Project workspace detected — overriding location-based classification".into());
        // Check git remote for accurate depends_on
        let has_remote = path.join(".git").exists()
            && std::process::Command::new("git")
                .args(["-C", &path.to_string_lossy(), "remote", "get-url", "origin"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        if has_remote {
            result.depends_on = "Remote git repository (origin configured)".to_string();
            result.regenerated_by =
                "git clone from remote — local uncommitted work not regenerable".to_string();
            result.deletion_impact =
                "Project source code permanently lost. Uncommitted work irrecoverable. Restore via git clone.".to_string();
        } else if path.join(".git").exists() {
            result.depends_on = "Local Git repository — no remote configured".to_string();
            result.regenerated_by =
                "Not regenerable — no remote to clone from, must restore from backup".to_string();
            result.deletion_impact =
                "Project source code and Git history permanently lost. Cannot be recovered."
                    .to_string();
        } else {
            result.depends_on = "None".to_string();
            result.regenerated_by =
                "Not regenerable — project must be recreated or cloned".to_string();
            result.deletion_impact =
                "Project source code permanently lost. Must restore from backup or VCS remote."
                    .to_string();
        }
        result.created_by = "Developer".to_string();
        // Keep matched = true, updated category
    }

    // ── Layer 2.5: Project/workspace detection (filesystem-aware) ──
    // Only when no rule matched and path is a directory.
    // This is expensive (filesystem access) but only called from explain,
    // not from the scan pipeline (which uses classify_fast).
    if !matched && path.is_dir() {
        if path.join("Cargo.toml").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-rust".to_string();
            result
                .reasons
                .push("Rust project workspace detected (Cargo.toml present)".into());
            matched = true;
        } else if path.join("package.json").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-node".to_string();
            result
                .reasons
                .push("Node.js project workspace detected (package.json present)".into());
            matched = true;
        } else if path.join("go.mod").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-go".to_string();
            result
                .reasons
                .push("Go project workspace detected (go.mod present)".into());
            matched = true;
        } else if path.join("pyproject.toml").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-python".to_string();
            result
                .reasons
                .push("Python project workspace detected (pyproject.toml present)".into());
            matched = true;
        } else if path.join(".git").exists() {
            result.category = Category::ProjectWorkspace;
            result.risk_level = RiskLevel::High;
            result.regenerable = false;
            result.matched_by = "project-git".to_string();
            result
                .reasons
                .push("Git repository detected (.git directory present)".into());
            // Check if this has a remote origin (clone) or is local-only (init)
            let has_remote = std::process::Command::new("git")
                .args(["-C", &path.to_string_lossy(), "remote", "get-url", "origin"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if has_remote {
                result.depends_on = "Remote git repository (origin configured)".to_string();
                result.regenerated_by =
                    "git clone from remote — local uncommitted work not regenerable".to_string();
                result.deletion_impact =
                    "Project source code permanently lost. Uncommitted work irrecoverable. Restore via git clone.".to_string();
            } else {
                result.depends_on = "Local Git repository — no remote configured".to_string();
                result.regenerated_by =
                    "Not regenerable — no remote to clone from, must restore from backup"
                        .to_string();
                result.deletion_impact =
                    "Project source code and entire Git history permanently lost. Cannot be recovered.".to_string();
            }
            result.created_by = "git init / git clone".to_string();
            matched = true;
        }
    }

    // ── Layer 2: Metadata analysis ────────────────────────────
    if metadata::is_elf_binary(path) {
        if result.category == Category::Unknown {
            result.category = Category::SystemBinary;
            result.risk_level = RiskLevel::Critical;
            result.reasons.push("ELF binary detected".into());
        }
        result.confidence += 0.3;
    }

    if metadata::is_regular_executable(path) && !path_str.ends_with(".sh") {
        result.reasons.push("Executable permission set".into());
        result.confidence += 0.1;
    }

    // ── Layer 3: Regenerability analysis ──────────────────────
    if !matched && result.category == Category::Unknown {
        // Check if path looks regenerable
        if lower.contains("/cache/") || lower.contains("/tmp/") {
            result.category = Category::Cache;
            result.risk_level = RiskLevel::Low;
            result.regenerable = true;
            result
                .reasons
                .push("Cache directory pattern detected".into());
            result.confidence += 0.5;
        }
    }

    // ── Layer 3.5: Parent-child context inheritance ────────────
    // v7.2: When a path is still Unknown, walk up to find a classified
    // parent directory and inherit its category with reduced confidence.
    // Fixes: target/debug, target/release, and any other children of known artifacts.
    if result.category == Category::Unknown {
        if let Some(ancestor_classification) = classify_ancestor(path) {
            result.category = ancestor_classification.0;
            result.risk_level = ancestor_classification.1;
            result.regenerable = ancestor_classification.2;
            result.matched_by = ancestor_classification.3;
            result.reasons.clear();
            result.reasons.push(format!(
                "Inherited from parent directory classification ({})",
                result.category.display()
            ));
            // Inherit intel fields from parent
            result.created_by = ancestor_classification.4;
            result.regenerated_by = ancestor_classification.5;
            result.depends_on = ancestor_classification.6;
            result.deletion_impact = ancestor_classification.7;
            matched = true;
            // Reduced confidence for inherited classification
            result.confidence = 0.55;
        }
    }

    // ── Layer 4: Confidence scoring ───────────────────────────
    if matched {
        result.confidence = result.confidence.max(0.85); // Rule match = high confidence
    }

    // Boost confidence for regenerable items with cache-like paths
    if result.regenerable && result.confidence < 0.6 {
        result.confidence += 0.2;
    }

    // Cap confidence
    result.confidence = result.confidence.clamp(0.0, 1.0);

    // v6.3: Numerical confidence scoring
    super::confidence::score(&mut result, path, &lower);

    // If still unknown, note it
    if result.category == Category::Unknown {
        result.reasons.push("No classification rule matched".into());
    }

    // v7.1: Generate classification reasoning — why this category?
    result.classification_reasoning = crate::dependency::generate_reasoning(&result);

    result
}

/// Check if a directory contains project markers.
/// Detects: Cargo.toml, package.json, go.mod, pyproject.toml, .git.
fn project_markers_found(path: &Path) -> bool {
    path.join("Cargo.toml").exists()
        || path.join("package.json").exists()
        || path.join(".git").exists()
        || path.join("go.mod").exists()
        || path.join("pyproject.toml").exists()
}

/// Cached ancestor classification result for inheritance.
type AncestorClassification = (
    Category,
    RiskLevel,
    bool,
    String,
    String,
    String,
    String,
    String,
);

/// Try to classify ancestor directories of a path.
/// Walks up at most 5 levels, returning the first classified ancestor's metadata.
fn classify_ancestor(path: &Path) -> Option<AncestorClassification> {
    let rules = super::rules::rule_database();
    let mut ancestor = path.parent()?;
    for _ in 0..5 {
        let lower = ancestor.to_string_lossy().to_lowercase();
        for rule in rules {
            if (rule.matches)(ancestor, &lower) {
                return Some((
                    rule.category,
                    rule.risk_level,
                    rule.regenerable,
                    format!("inherit-{}", rule.name),
                    rule.created_by.to_string(),
                    rule.regenerated_by.to_string(),
                    rule.depends_on.to_string(),
                    rule.deletion_impact.to_string(),
                ));
            }
        }
        match ancestor.parent() {
            Some(parent) => ancestor = parent,
            None => break,
        }
    }
    None
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
