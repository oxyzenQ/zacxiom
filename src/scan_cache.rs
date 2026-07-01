// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Incremental scan cache (v14.1) — cache-aware classification engine.
//!
//! v14.1: Stores full classification results (decision, risk_score, engine_category)
//! so repeat scans skip the entire classification pipeline for unchanged files.
//! On 85k files with no changes: classify() does 0 CPU work — just HashMap lookups.
//!
//! Cache location: ~/.cache/zacxiom/scan_cache.json
//! (XDG_CACHE_HOME — this IS disposable cache, unlike snapshots)
//!
//! # How it works
//!
//! 1. First scan: classify every file, store result in cache
//! 2. Repeat scan: for each file, check (path, size, mtime) in cache
//!    - HIT: reuse cached classification (skip risk scoring + engine rules)
//!    - MISS: classify normally, store new result
//! 3. `--no-cache` forces full reclassification
//!
//! # Cache invalidation
//!
//! A cache entry is valid only if:
//! - File size matches (content changed)
//! - File mtime matches (metadata changed)
//!
//! If either differs, the entry is stale and must be reclassified.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

/// v14.1: Cache version — bumped from 1 to 2 (added classification fields).
pub const CACHE_VERSION: u32 = 2;

/// Cached file entry with full classification result.
/// Used to skip reclassification on unchanged files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    pub path: String,
    pub size: u64,
    pub mtime_secs: u64,
    pub inode: u64,
    // v14.1: Classification cache — reuse if size+mtime unchanged
    pub decision: String,
    pub risk_score: f64,
    pub engine_category: String,
    pub engine_confidence: u8,
    pub cache_domain: String, // CacheDomain as string
}

/// In-memory cache map: path → cached metadata + classification
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanCache {
    pub version: u32,
    pub files: HashMap<String, CachedFile>,
    pub last_updated: u64,
}

impl Default for ScanCache {
    fn default() -> Self {
        ScanCache::new()
    }
}

/// v14.1: Global cache hit/miss counters (atomic for thread-safe classification)
pub static CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
pub static CACHE_MISSES: AtomicUsize = AtomicUsize::new(0);

/// Reset hit/miss counters (called at start of each classify() call)
pub fn reset_stats() {
    CACHE_HITS.store(0, Ordering::Relaxed);
    CACHE_MISSES.store(0, Ordering::Relaxed);
}

/// Get cache stats (hits, misses, hit_rate_pct)
pub fn get_stats() -> (usize, usize, f64) {
    let hits = CACHE_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);
    let total = hits + misses;
    let rate = if total > 0 {
        (hits as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    (hits, misses, rate)
}

impl ScanCache {
    pub fn new() -> Self {
        ScanCache {
            version: CACHE_VERSION,
            files: HashMap::new(),
            last_updated: 0,
        }
    }

    /// Load cache from disk. Returns empty cache if file doesn't exist or is corrupt.
    /// v14.1: If version mismatch, start fresh (old cache incompatible).
    pub fn load() -> ScanCache {
        let path = cache_path();
        if !path.exists() {
            return ScanCache::new();
        }
        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => return ScanCache::new(),
        };
        match serde_json::from_str::<ScanCache>(&data) {
            Ok(c) if c.version == CACHE_VERSION => c,
            _ => ScanCache::new(), // Wrong version or corrupt — start fresh
        }
    }

    /// Save cache to disk (atomic: write to temp, then rename).
    pub fn save(&self) {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(self) {
            let tmp = path.with_extension("tmp");
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }

    /// v14.1: Check if a file has a valid cache entry (size + mtime match).
    /// Returns Some(&CachedFile) if cache hit, None if miss.
    pub fn check_hit(&self, path: &str, size: u64, mtime_secs: u64) -> Option<&CachedFile> {
        if let Some(cached) = self.files.get(path) {
            if cached.size == size && cached.mtime_secs == mtime_secs {
                CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                return Some(cached);
            }
        }
        CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Get a cache entry by path (for diff display — no hit/miss tracking).
    pub fn get(&self, path: &str) -> Option<&CachedFile> {
        self.files.get(path)
    }

    /// Insert or update a file in cache (with classification result).
    #[allow(clippy::too_many_arguments)]
    pub fn insert_classified(
        &mut self,
        path: &str,
        size: u64,
        mtime_secs: u64,
        inode: u64,
        decision: &str,
        risk_score: f64,
        engine_category: &str,
        engine_confidence: u8,
        cache_domain: &str,
    ) {
        self.files.insert(
            path.to_string(),
            CachedFile {
                path: path.to_string(),
                size,
                mtime_secs,
                inode,
                decision: decision.to_string(),
                risk_score,
                engine_category: engine_category.to_string(),
                engine_confidence,
                cache_domain: cache_domain.to_string(),
            },
        );
    }

    /// Remove entries that no longer exist on disk.
    pub fn prune_missing(&mut self) -> usize {
        let before = self.files.len();
        self.files.retain(|path, _| Path::new(path).exists());
        before - self.files.len()
    }
}

/// Get inode number for a file (Unix).
#[cfg(unix)]
pub fn get_inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).map(|m| m.ino()).unwrap_or(0)
}

#[cfg(not(unix))]
pub fn get_inode(_path: &Path) -> u64 {
    0
}

/// Get mtime in seconds since UNIX_EPOCH.
pub fn get_mtime_secs(path: &Path) -> Option<u64> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn cache_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("zacxiom/scan_cache.json")
    } else {
        let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
        PathBuf::from(home).join(".cache/zacxiom/scan_cache.json")
    }
}

pub fn cache_dir() -> PathBuf {
    cache_path()
        .parent()
        .unwrap_or_else(|| Path::new(".cache/zacxiom"))
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_new_empty() {
        let cache = ScanCache::new();
        assert_eq!(cache.version, CACHE_VERSION);
        assert!(cache.files.is_empty());
    }

    #[test]
    fn test_cache_hit_miss() {
        let mut cache = ScanCache::new();
        reset_stats();
        cache.insert_classified(
            "/tmp/test",
            100,
            12345,
            1,
            "Safe",
            0.05,
            "Cache",
            95,
            "browser",
        );
        // HIT: size + mtime match
        let hit = cache.check_hit("/tmp/test", 100, 12345);
        assert!(hit.is_some());
        // MISS: size changed
        let miss1 = cache.check_hit("/tmp/test", 200, 12345);
        assert!(miss1.is_none());
        // MISS: mtime changed
        let miss2 = cache.check_hit("/tmp/test", 100, 99999);
        assert!(miss2.is_none());
        // MISS: path not in cache
        let miss3 = cache.check_hit("/nonexistent", 100, 12345);
        assert!(miss3.is_none());
        let (hits, misses, _) = get_stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 3);
    }

    #[test]
    fn test_cache_save_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("XDG_CACHE_HOME");

        let mut cache = ScanCache::new();
        cache.insert_classified("/tmp/foo", 42, 999, 7, "Safe", 0.0, "Cache", 100, "browser");
        cache.save();

        let loaded = ScanCache::load();
        assert_eq!(loaded.version, CACHE_VERSION);
        assert_eq!(loaded.files.len(), 1);
        let entry = loaded.files.get("/tmp/foo").unwrap();
        assert_eq!(entry.decision, "Safe");
        assert_eq!(entry.risk_score, 0.0);

        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_cache_version_invalidation() {
        // v1 cache should be ignored (version mismatch)
        let tmp = TempDir::new().unwrap();
        let old_home = std::env::var_os("HOME");
        let old_xdg = std::env::var_os("XDG_CACHE_HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("XDG_CACHE_HOME");

        // Write a v1 cache (create parent dir first)
        let cpath = cache_path();
        fs::create_dir_all(cpath.parent().unwrap()).unwrap();
        let v1_json = r#"{"version":1,"files":{},"last_updated":0}"#;
        fs::write(&cpath, v1_json).unwrap();

        // Load should return fresh cache (v2)
        let loaded = ScanCache::load();
        assert_eq!(loaded.version, CACHE_VERSION);
        assert!(loaded.files.is_empty());

        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        match old_xdg {
            Some(h) => std::env::set_var("XDG_CACHE_HOME", h),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }
    }

    #[test]
    fn test_cache_stats_reset() {
        reset_stats();
        let (h, m, _) = get_stats();
        assert_eq!(h, 0);
        assert_eq!(m, 0);
    }
}
