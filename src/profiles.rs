// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! System health profiling and profile modes.
//!
//! v3: Adapts behavior based on system disk state and user profile selection.
//! Profiles: minimal, dev, gaming, server. Auto-detects low-disk/recovery mode.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum HealthMode {
    /// Normal operation, disk >20% free
    Normal,
    /// Low disk: <20% free — aggressive cache scanning enabled
    LowDisk,
    /// Recovery: <5% free — urgent mode, broader scan
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Profile {
    /// Minimal: only browser + system caches
    Minimal,
    /// Dev: includes build artifacts + developer caches
    Dev,
    /// Gaming: includes shader caches, game data
    Gaming,
    /// Server: focuses on log rotation, package caches
    Server,
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Profile::Minimal => write!(f, "minimal"),
            Profile::Dev => write!(f, "dev"),
            Profile::Gaming => write!(f, "gaming"),
            Profile::Server => write!(f, "server"),
        }
    }
}

#[allow(dead_code)]
impl Profile {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "minimal" => Profile::Minimal,
            "dev" => Profile::Dev,
            "gaming" => Profile::Gaming,
            "server" => Profile::Server,
            _ => Profile::Dev,
        }
    }

    /// Whether this profile should scan build artifact directories.
    pub fn scan_build_artifacts(&self) -> bool {
        matches!(self, Profile::Dev | Profile::Gaming)
    }

    /// Whether this profile should scan developer tool caches.
    pub fn scan_dev_caches(&self) -> bool {
        matches!(self, Profile::Dev)
    }

    /// Whether to scan shader/game caches.
    pub fn scan_gaming_caches(&self) -> bool {
        matches!(self, Profile::Gaming)
    }

    /// Whether to scan log directories.
    pub fn scan_logs(&self) -> bool {
        matches!(self, Profile::Server | Profile::Gaming)
    }
}

/// Detect system health based on disk usage of the root filesystem.
pub fn detect_health() -> HealthMode {
    detect_health_at("/")
}

fn detect_health_at(mount: &str) -> HealthMode {
    // Use statvfs via a simple df-like check
    #[cfg(target_os = "linux")]
    {
        if let Some(stat) = nix_statvfs(mount) {
            let total = stat.total_blocks * stat.block_size;
            let free = stat.available_blocks * stat.block_size;
            if total == 0 {
                return HealthMode::Normal;
            }
            let pct_free = (free as f64) / (total as f64);
            if pct_free < 0.05 {
                return HealthMode::Recovery;
            }
            if pct_free < 0.20 {
                return HealthMode::LowDisk;
            }
        }
    }
    HealthMode::Normal
}

#[cfg(target_os = "linux")]
struct Statvfs {
    total_blocks: u64,
    available_blocks: u64,
    block_size: u64,
}

#[cfg(target_os = "linux")]
fn nix_statvfs(path: &str) -> Option<Statvfs> {
    use std::mem;
    let path = std::ffi::CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { mem::zeroed() };
    let ret = unsafe { libc::statvfs(path.as_ptr(), &mut stat) };
    if ret == 0 {
        Some(Statvfs {
            total_blocks: stat.f_blocks,
            available_blocks: stat.f_bavail,
            block_size: stat.f_frsize as u64,
        })
    } else {
        None
    }
}

/// Estimate disk gain from cleaning files of total size.
pub fn estimate_disk_gain(safe_size: u64, low_risk_size: u64) -> DiskGainEstimate {
    let health = detect_health();
    let total = safe_size + low_risk_size;

    let recommendation = match health {
        HealthMode::Normal => {
            if total > 1_073_741_824 {
                "Significant space to reclaim"
            } else {
                "Minor cleanup available"
            }
        }
        HealthMode::LowDisk => "Recommended — disk space is low",
        HealthMode::Recovery => "URGENT — critically low disk space",
    };

    DiskGainEstimate {
        safe_reclaimable: safe_size,
        low_risk_reclaimable: low_risk_size,
        total_reclaimable: total,
        health_mode: format!("{:?}", health),
        recommendation: recommendation.to_string(),
    }
}

#[derive(Debug, Serialize)]
pub struct DiskGainEstimate {
    pub safe_reclaimable: u64,
    pub low_risk_reclaimable: u64,
    pub total_reclaimable: u64,
    pub health_mode: String,
    pub recommendation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_from_str() {
        assert_eq!(Profile::from_str("minimal"), Profile::Minimal);
        assert_eq!(Profile::from_str("DEV"), Profile::Dev);
        assert_eq!(Profile::from_str("unknown"), Profile::Dev);
    }

    #[test]
    fn test_profile_permissions() {
        let dev = Profile::Dev;
        assert!(dev.scan_build_artifacts());
        assert!(dev.scan_dev_caches());

        let minimal = Profile::Minimal;
        assert!(!minimal.scan_build_artifacts());
        assert!(!minimal.scan_dev_caches());

        let gaming = Profile::Gaming;
        assert!(gaming.scan_gaming_caches());
        assert!(gaming.scan_logs());
    }

    #[test]
    fn test_disk_gain_estimate() {
        let estimate = estimate_disk_gain(500_000_000, 200_000_000);
        assert_eq!(estimate.safe_reclaimable, 500_000_000);
        assert_eq!(estimate.total_reclaimable, 700_000_000);
        assert!(!estimate.recommendation.is_empty());
    }
}
