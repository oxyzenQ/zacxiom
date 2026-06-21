// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Context memory engine — adaptive risk thresholds per system.
//!
//! Remembers system behavior patterns over time:
//! - What user usually deletes → lower future risk for those paths
//! - What causes system issues → raise future risk
//! - Adaptive thresholds based on system history

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ContextMemory {
    /// Paths user has deleted multiple times without issues.
    pub trusted_paths: HashMap<String, TrustRecord>,
    /// Paths that caused issues (user reverted via undo).
    pub flagged_paths: HashMap<String, FlagRecord>,
    /// Total clean sessions.
    pub sessions: u64,
    /// System-specific risk offset (adjusts thresholds).
    pub risk_offset: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustRecord {
    pub clean_count: u64,
    pub last_cleaned: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlagRecord {
    pub flag_count: u64,
    pub last_flagged: String,
    pub reason: String,
}

#[allow(dead_code)]
impl ContextMemory {
    pub fn load() -> Self {
        let path = memory_path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            ContextMemory::default()
        }
    }

    pub fn save(&self) {
        let path = memory_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, json);
        }
    }

    /// Record a successful clean of a path.
    pub fn record_clean(&mut self, paths: &[String]) {
        self.sessions += 1;
        let ts = epoch_ts();
        for path in paths {
            self.trusted_paths
                .entry(path.clone())
                .and_modify(|r| {
                    r.clean_count += 1;
                    r.last_cleaned = ts.clone();
                })
                .or_insert(TrustRecord {
                    clean_count: 1,
                    last_cleaned: ts.clone(),
                });
        }
        // Adjust risk offset: more trusted paths → slightly lower risk
        self.risk_offset = -(self.trusted_paths.len() as f64 * 0.001).min(0.05);
        self.save();
    }

    /// Flag a path as problematic (called after undo).
    pub fn flag_path(&mut self, path: &str, reason: &str) {
        self.flagged_paths
            .entry(path.to_string())
            .and_modify(|r| {
                r.flag_count += 1;
                r.last_flagged = epoch_ts();
                r.reason = reason.to_string();
            })
            .or_insert(FlagRecord {
                flag_count: 1,
                last_flagged: epoch_ts(),
                reason: reason.to_string(),
            });
        // Increase risk offset when paths are flagged
        self.risk_offset = (self.flagged_paths.len() as f64 * 0.01).min(0.1);
        self.save();
    }

    /// Get adaptive risk modifier for a path.
    /// Negative = lower risk (trusted), Positive = higher risk (flagged).
    pub fn risk_modifier(&self, path: &str) -> f64 {
        let mut modifier = 0.0;

        if let Some(trust) = self.trusted_paths.get(path) {
            // Each trusted clean reduces risk slightly, max -0.08
            modifier -= (trust.clean_count as f64 * 0.005).min(0.08);
        }

        if let Some(flag) = self.flagged_paths.get(path) {
            // Each flag increases risk, max +0.15
            modifier += (flag.flag_count as f64 * 0.03).min(0.15);
        }

        modifier + self.risk_offset
    }

    /// Check if the system has stabilized (enough clean history).
    pub fn is_stabilized(&self) -> bool {
        self.sessions >= 5
    }
}

fn memory_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".cache/zacxiom/memory.json")
}

fn epoch_ts() -> String {
    format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_memory_is_empty() {
        let m = ContextMemory::default();
        assert_eq!(m.sessions, 0);
        assert!(!m.is_stabilized());
    }

    #[test]
    fn test_record_clean_builds_trust() {
        let mut m = ContextMemory::default();
        m.record_clean(&["/tmp/test1".into(), "/tmp/test2".into()]);
        assert_eq!(m.sessions, 1);
        assert!(m.risk_modifier("/tmp/test1") < 0.0); // trusted
    }

    #[test]
    fn test_flag_path_increases_risk() {
        let mut m = ContextMemory::default();
        m.flag_path("/tmp/bad", "system unstable");
        assert!(m.risk_modifier("/tmp/bad") > 0.0);
    }

    #[test]
    fn test_stabilized_after_5_sessions() {
        let mut m = ContextMemory::default();
        for _ in 0..5 {
            m.record_clean(&["/tmp/x".into()]);
        }
        assert!(m.is_stabilized());
    }
}
