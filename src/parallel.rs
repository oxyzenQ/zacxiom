// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Parallel-safe Analysis — v8.6 → v11.1 (dead deletion path removed)
//!
//! Concurrent analysis with sequential execution guarantees.
//!
//! Analysis (classification, planning, advisor) MAY run concurrently.
//! Cleanup execution is ALWAYS delegated to the single trash/recovery pipeline.
//! There is exactly one deletion implementation: cleaner::clean().
//!
//! Architecture:
//!   analyze_concurrently() → produces plans in parallel (read-only)
//!   validate_no_overlap()  → safety check before cleanup

use crate::planner::CleanupPlan;
use std::path::PathBuf;

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
