// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — Ownership detection, risk scoring, and size computation.
//!
//! v8.6: Improved reclaim estimation with metadata caching and fast-path heuristics.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::discovery;
use crate::engine::{types::RiskLevel, Category, ClassificationResult};
use crate::impact;

/// Cache for directory size computations.
/// Keyed by canonical path, valid for 60 seconds.
static SIZE_CACHE: std::sync::LazyLock<Mutex<HashMap<PathBuf, (SystemTime, u64)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

const CACHE_TTL: Duration = Duration::from_secs(60);

/// Invalidate the size cache (useful for testing).
pub(crate) fn invalidate_size_cache() {
    if let Ok(mut cache) = SIZE_CACHE.lock() {
        cache.clear();
    }
}

/// Boost confidence when project ownership is discovered.
pub(crate) fn boost_confidence_from_discovery(eng: &mut ClassificationResult) {
    if let Some(project) = discovery::find_project_for_path(&eng.path) {
        if eng.confidence_score < 95 {
            eng.confidence_score = (eng.confidence_score + 10).min(99);
        }
        let reason = format!(
            "Project ownership discovered: {} ({})",
            project.name,
            project.ecosystem.display()
        );
        if !eng.confidence_reasons.contains(&reason) {
            eng.confidence_reasons.push(reason);
        }
    }
}

/// Compute risk level from engine classification and impact analysis.
pub(crate) fn compute_risk(
    eng: &ClassificationResult,
    impact_analysis: &impact::ImpactAnalysis,
) -> RiskLevel {
    // Map engine risk level directly for protected/critical categories
    if eng.category.is_protected() || matches!(eng.category, Category::UserHomeRoot) {
        return RiskLevel::Critical;
    }

    // Configuration categories store user-curated settings — deleting loses
    // customization.  They are never safe to clean, so the minimum risk
    // must be High (not Moderate) to stay consistent with the safety verdict.
    if matches!(
        eng.category,
        Category::ApplicationConfiguration
            | Category::ShellConfiguration
            | Category::EnvironmentFile
    ) {
        return RiskLevel::High;
    }

    // Map from impact level to our risk level
    match impact_analysis.level {
        impact::ImpactLevel::Critical => RiskLevel::Critical,
        impact::ImpactLevel::High => RiskLevel::High,
        impact::ImpactLevel::Medium => RiskLevel::Moderate,
        impact::ImpactLevel::Low => {
            if eng.category.is_cleanable() {
                RiskLevel::Low
            } else {
                RiskLevel::Moderate
            }
        }
    }
}

/// Compute the estimated size of a path in bytes.
///
/// v8.6: Uses metadata caching to avoid redundant directory walks.
pub(crate) fn compute_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    // Fast-path: single file
    if path.is_file() {
        return fs::symlink_metadata(path).map(|m| m.len()).unwrap_or(0);
    }

    // Check cache
    if let Ok(canonical) = path.canonicalize() {
        if let Ok(cache) = SIZE_CACHE.lock() {
            if let Some((ts, size)) = cache.get(&canonical) {
                if ts.elapsed().unwrap_or(Duration::MAX) < CACHE_TTL {
                    return *size;
                }
            }
        }
    }

    let size = walk_dir_size(path);

    // Update cache
    if let Ok(canonical) = path.canonicalize() {
        if let Ok(mut cache) = SIZE_CACHE.lock() {
            cache.insert(canonical, (SystemTime::now(), size));
        }
    }

    size
}

/// Recursive directory walk for accurate size computation.
fn walk_dir_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entrypath = entry.path();
            if entrypath.is_file() {
                total += fs::symlink_metadata(&entrypath)
                    .map(|m| m.len())
                    .unwrap_or(0);
            } else if entrypath.is_dir() {
                total += walk_dir_size(&entrypath);
            }
        }
    }
    total
}

/// Estimate reclaimable bytes using fast path when possible.
/// Returns the estimate and a confidence flag (true = accurate, false = approximate).
pub(crate) fn estimate_reclaim_fast(path: &Path) -> (u64, bool) {
    if !path.exists() {
        return (0, true);
    }

    if path.is_file() {
        return (
            fs::symlink_metadata(path).map(|m| m.len()).unwrap_or(0),
            true,
        );
    }

    // For directories, always do an accurate walk (with caching)
    (compute_size(path), true)
}
