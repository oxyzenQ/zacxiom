// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Structured rule database — replaces giant if/else chains.
//!
//! Rules are ordered by priority. First match wins.
//! Each rule specifies a path pattern, resulting category, and risk level.
//!
//! v7: Rules carry artifact intelligence — ownership, regeneration,
//! dependency, and deletion impact metadata.

use super::types::{Category, RiskLevel};
use std::path::Path;
use std::sync::OnceLock;

/// A single classification rule.
///
/// v7: Enriched with artifact intelligence fields.
pub struct Rule {
    pub name: &'static str,
    /// Match logic: returns true if this rule applies to the given path.
    pub matches: fn(&Path, &str) -> bool,
    pub category: Category,
    pub risk_level: RiskLevel,
    pub regenerable: bool,
    pub reason: &'static str,
    // ── v7: Artifact Intelligence fields ──────────────────────
    /// Who created this artifact? (e.g. "Cargo", "Rustup", "npm", "Browser")
    pub created_by: &'static str,
    /// How to regenerate this artifact? (e.g. "cargo build", "rustup toolchain install")
    pub regenerated_by: &'static str,
    /// What does this artifact depend on? (e.g. "Cargo.toml", "package.json")
    pub depends_on: &'static str,
    /// What happens if this artifact is deleted?
    pub deletion_impact: &'static str,
}

/// Build the full rule database in priority order.
/// Cached via OnceLock — called once, shared across all classify() invocations.
/// Priority: system-protected > home-critical > config > cache > app-specific > fallback.
pub fn rule_database() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(build_rules)
}

