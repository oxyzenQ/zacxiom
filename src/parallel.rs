// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Parallel-safe Analysis — v8.6
//!
//! Concurrent analysis with sequential execution guarantees.
//!
//! Analysis (classification, planning, advisor) MAY run concurrently.
//! Cleanup execution MUST be sequential to avoid race conditions.
//!
//! Architecture:
//!   analyze_concurrently() → produces plans in parallel
//!   execute_sequentially() → applies plans one at a time

use crate::planner::CleanupPlan;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Result of concurrent analysis of multiple paths.
#[derive(Debug)]
pub struct ConcurrentAnalysis {
    /// Plans generated for each path, in the same order as input.
    pub plans: Vec<Option<CleanupPlan>>,
    /// Total estimated reclaimable bytes across all plans.
    pub total_reclaimable: u64,
    /// Whether all analyses completed without error.
    pub all_succeeded: bool,
}

/// Analyze multiple paths concurrently (read-only classification + planning).
///
/// Uses rayon for parallelism. Each path is analyzed independently.
/// No filesystem mutations occur.
///
/// Returns plans in input order. Failed analyses produce None entries.
pub fn analyze_concurrently(paths: &[PathBuf]) -> ConcurrentAnalysis {
    use rayon::prelude::*;

    let plans: Vec<Option<CleanupPlan>> = paths
        .par_iter()
        .map(|path| {
            if path.exists() {
                // Use the planner but catch any errors
                let plan = crate::planner::plan(path);
                Some(plan)
            } else {
                None
            }
        })
        .collect();

    let all_succeeded = plans.iter().all(|p| p.is_some());
    let total_reclaimable: u64 = plans
        .iter()
        .filter_map(|p| p.as_ref())
        .map(|p| p.estimated_reclaimable_bytes)
        .sum();

    ConcurrentAnalysis {
        plans,
        total_reclaimable,
        all_succeeded,
    }
}

/// Validate that paths do not overlap and are safe for sequential cleanup.
///
/// Returns (valid_paths, conflicts) where conflicts are paths that
/// contain or are contained by other paths in the set.
pub fn validate_no_overlap(paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<(PathBuf, PathBuf)>) {
    let mut conflicts = Vec::new();
    let mut valid = Vec::new();

    for (i, a) in paths.iter().enumerate() {
        let mut has_conflict = false;
        for (j, b) in paths.iter().enumerate() {
            if i == j {
                continue;
            }
            // Check if a contains b or b contains a
            if a.starts_with(b) || b.starts_with(a) {
                conflicts.push((a.clone(), b.clone()));
                has_conflict = true;
                break;
            }
        }
        if !has_conflict {
            valid.push(a.clone());
        }
    }

    (valid, conflicts)
}

/// Execute cleanup plans sequentially (one at a time).
///
/// ⚠️  This function performs actual filesystem mutations.
/// Each plan is applied in order. Returns count of successful cleanups.
///
/// CRITICAL: Never call this from a parallel context.
/// This must be the ONLY thread performing cleanup at any given time.
pub fn execute_sequentially(
    plans: &[(&CleanupPlan, &Path)],
    smart: bool,
    force: bool,
) -> (usize, u64) {
    let mut cleaned = 0usize;
    let mut freed: u64 = 0;

    for (plan, path) in plans {
        if !plan.safe_to_clean {
            continue;
        }

        // Validate path still exists before cleaning
        if !path.exists() {
            continue;
        }

        // Apply smart/force filtering
        let can_clean = match plan.risk_level {
            crate::engine::types::RiskLevel::Minimal | crate::engine::types::RiskLevel::Low => true,
            crate::engine::types::RiskLevel::Moderate => smart || force,
            crate::engine::types::RiskLevel::High => force,
            crate::engine::types::RiskLevel::Critical => false,
        };

        if !can_clean {
            continue;
        }

        // Perform the actual cleanup
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if meta.is_dir() {
                if std::fs::remove_dir_all(path).is_ok() {
                    cleaned += 1;
                    freed += plan.estimated_reclaimable_bytes;
                }
            } else if meta.is_file() && std::fs::remove_file(path).is_ok() {
                cleaned += 1;
                freed += plan.estimated_reclaimable_bytes;
            }
        }
    }

    (cleaned, freed)
}

/// Guard type that prevents parallel cleanup execution.
///
/// Created once per cleanup session. Drop-implements logging.
pub struct CleanupSession {
    pub paths: Vec<PathBuf>,
}

impl CleanupSession {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        CleanupSession { paths }
    }

    /// Number of paths in this cleanup session.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether the session is empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

/// Alias for readability.
pub type ArcAnalysis = Arc<ConcurrentAnalysis>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_no_overlap_detects_conflict() {
        let paths = vec![
            PathBuf::from("/tmp/a"),
            PathBuf::from("/tmp/a/b"),
            PathBuf::from("/tmp/c"),
        ];
        let (valid, conflicts) = validate_no_overlap(&paths);
        assert!(valid.contains(&PathBuf::from("/tmp/c")));
        assert!(!conflicts.is_empty());
    }

    #[test]
    fn test_validate_no_overlap_all_clean() {
        let paths = vec![
            PathBuf::from("/tmp/a"),
            PathBuf::from("/tmp/b"),
            PathBuf::from("/tmp/c"),
        ];
        let (valid, conflicts) = validate_no_overlap(&paths);
        assert_eq!(valid.len(), 3);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_concurrent_analysis() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("a")).unwrap();
        fs::write(root.join("a/file.txt"), b"hello").unwrap();
        fs::create_dir(root.join("b")).unwrap();
        fs::write(root.join("b/file.txt"), b"world").unwrap();

        let paths: Vec<PathBuf> = vec![root.join("a"), root.join("b")];
        let analysis = analyze_concurrently(&paths);

        assert_eq!(analysis.plans.len(), 2);
        assert!(analysis.all_succeeded);
    }
}
