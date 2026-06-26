// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner types and constants.

use std::path::Path;

use crate::engine::types::RiskLevel;

/// A cleanup recommendation for a given path.
///
/// This is a read-only advisory. It never triggers deletion.
#[derive(Debug, Clone)]
pub struct CleanupPlan {
    /// Is this path safe to clean?
    pub safe_to_clean: bool,
    /// Risk level for cleanup.
    pub risk_level: RiskLevel,
    /// Estimated reclaimable space in bytes.
    pub estimated_reclaimable_bytes: u64,
    /// Human-readable recommendation (what to do).
    pub recommendation: String,
    /// Why this recommendation (distinct from recommendation — no duplication).
    pub reason: String,
    /// How to regenerate the content after cleaning.
    pub regeneration: String,
    /// Suggested ecosystem-aware cleanup commands (never raw `rm -rf`).
    pub suggested_commands: Vec<String>,
    /// Additional notes and caveats.
    pub notes: Vec<String>,
    /// If unsafe, suggest safer child directories that actually exist.
    pub safer_alternatives: Vec<String>,
    /// Contextual expected result — path-aware wording.
    pub expected_result: String,
}

/// System-critical paths that must never be planned.
pub(crate) static DANGEROUS_PATHS: &[&str] = &[
    "/", "/home", "/usr", "/etc", "/var", "/boot", "/root", "/sys", "/proc", "/dev", "/run",
];

/// Check if a path is a dangerous system path that must be blocked.
pub(crate) fn is_dangerous_system_path(path: &Path) -> Option<&'static str> {
    let raw = path.to_string_lossy();
    if raw == "/" {
        return Some("/");
    }
    let normalized = raw.trim_end_matches('/');
    DANGEROUS_PATHS.iter().find(|b| normalized == **b).copied()
}

/// Error returned when a dangerous path is blocked.
pub struct BlockedPath {
    pub path: String,
    pub reason: String,
    pub suggestions: Vec<&'static str>,
}
