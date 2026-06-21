// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Real risk scoring engine v3.
//!
//! v3: Meaningful risk differentiation using multiple signal inputs:
//! - cache domain (regenerability)
//! - file age (mtime)
//! - process handles (open files)
//! - package ownership
//! - system path sensitivity
//! - history trust
//! - memory adaptation
//!
//! Output: SAFE | LOW | MEDIUM | HIGH | CRITICAL (not flat 70%)

use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};
use std::collections::HashSet;
use std::path::PathBuf;

/// Risk input signals for the v3 engine.
pub struct RiskSignals<'a> {
    pub path: &'a str,
    pub size: u64,
    pub domain: &'a CacheDomain,
    pub ownership: &'a Ownership,
    pub open_files: Option<&'a HashSet<PathBuf>>,
    pub history_cleaned: Option<&'a HashSet<String>>,
    pub memory_modifier: f64,
    /// File age in days (None if unknown).
    pub age_days: Option<f64>,
}

/// Score a file using v3 real risk engine.
pub fn score_v3(signals: &RiskSignals) -> ClassifiedFile {
    let mut reasons: Vec<String> = Vec::new();
    let mut score = 0.0;

    // ── Signal 1: Cache regenerability (domain-based) ──
    // Can this cache be regenerated? If yes, lower risk.
    let regenerability = domain_regenerability(signals.domain);
    match regenerability {
        Regenerability::Fully => {
            score += 0.0;
            reasons.push("fully regenerable cache".into());
        }
        Regenerability::Partially => {
            score += 0.1;
            reasons.push("partially regenerable".into());
        }
        Regenerability::NotRegenerable => {
            score += 0.3;
            reasons.push("not regenerable — may contain user data".into());
        }
    }

    // ── Signal 2: File age ──
    // Older files = lower risk (user hasn't needed them recently)
    if let Some(age) = signals.age_days {
        if age > 90.0 {
            reasons.push(format!("aged {:.0}d — long unused", age));
            // score already set by regenerability — age confirms safety
        } else if age > 30.0 {
            reasons.push(format!("aged {:.0}d — moderately old", age));
            score += 0.02;
        } else if age < 1.0 {
            score += 0.15;
            reasons.push("recently modified (<1 day)".into());
        } else if age < 7.0 {
            score += 0.08;
            reasons.push("modified within 7 days".into());
        }
    }

    // ── Signal 3: Active process handles ──
    if let Some(open) = signals.open_files {
        let pb = PathBuf::from(signals.path);
        let resolved = if pb.exists() {
            pb.canonicalize().unwrap_or_else(|_| pb.clone())
        } else {
            pb.clone()
        };
        if open.contains(&pb) || open.contains(&resolved) {
            score += 0.5;
            reasons.push("open by running process".into());
        }
    }

    // ── Signal 4: Package ownership ──
    match signals.ownership {
        Ownership::Package { pkg_name } => {
            score += 0.05;
            reasons.push(format!("owned by package: {pkg_name}"));
        }
        Ownership::System => {
            score += 0.4;
            reasons.push("system-owned file".into());
        }
        Ownership::User { .. } => {
            // user-owned = lower risk baseline
        }
        Ownership::Orphan => {
            score += 0.02;
            reasons.push("no owning package".into());
        }
    }

    // ── Signal 5: System path sensitivity ──
    if signals.path.starts_with("/var/cache/") {
        // system cache — low risk, designed for cleanup
    } else if signals.path.starts_with("/tmp/") {
        score += 0.05;
        reasons.push("temporary directory".into());
    }

    // ── Signal 6: History trust ──
    if let Some(hist) = signals.history_cleaned {
        if hist.contains(signals.path) {
            score -= 0.04;
            reasons.push("previously cleaned without issue".into());
        }
    }

    // ── Signal 7: Memory adaptation ──
    score = (score + signals.memory_modifier).clamp(0.0, 1.0);

    // ── Determine decision from score ──
    let (decision, _risk_label) = if score < 0.12 {
        (Decision::Safe, "SAFE")
    } else if score < 0.30 {
        (Decision::LowRisk, "LOW")
    } else if score < 0.55 {
        (Decision::Moderate, "MEDIUM")
    } else if score < 0.80 {
        (Decision::HighRisk, "HIGH")
    } else {
        (Decision::Protected, "CRITICAL")
    };

    // Hard override: system-owned + unknown domain = protected
    if matches!(signals.ownership, Ownership::System)
        && matches!(signals.domain, CacheDomain::Unknown)
    {
        return ClassifiedFile {
            path: signals.path.to_string(),
            size: signals.size,
            cache_domain: signals.domain.clone(),
            ownership: signals.ownership.clone(),
            risk_score: 1.0,
            risk_reasons: vec!["system-owned, unknown cache domain — protected".into()],
            decision: Decision::Protected,
        };
    }

    ClassifiedFile {
        path: signals.path.to_string(),
        size: signals.size,
        cache_domain: signals.domain.clone(),
        ownership: signals.ownership.clone(),
        risk_score: score,
        risk_reasons: reasons,
        decision,
    }
}

#[derive(Debug)]
enum Regenerability {
    Fully,
    Partially,
    NotRegenerable,
}

fn domain_regenerability(domain: &CacheDomain) -> Regenerability {
    match domain {
        CacheDomain::Browser => Regenerability::Fully,
        CacheDomain::BuildArtifact => Regenerability::Fully,
        CacheDomain::PackageManager => Regenerability::Fully,
        CacheDomain::Developer => Regenerability::Fully,
        CacheDomain::System => Regenerability::Partially,
        CacheDomain::UserData => Regenerability::NotRegenerable,
        CacheDomain::Unknown => Regenerability::NotRegenerable,
    }
}

/// Get file age in days from mtime. Returns None if unavailable.
pub fn file_age_days(path: &str) -> Option<f64> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let age = mtime.elapsed().ok()?;
    Some(age.as_secs_f64() / 86400.0)
}

/// v1/v2 compatibility wrapper.
#[allow(dead_code)]
pub fn score(
    path: &str,
    size: u64,
    cache_domain: CacheDomain,
    ownership: Ownership,
) -> ClassifiedFile {
    score_v3(&RiskSignals {
        path,
        size,
        domain: &cache_domain,
        ownership: &ownership,
        open_files: None,
        history_cleaned: None,
        memory_modifier: 0.0,
        age_days: None,
    })
}

/// v2 compatibility wrapper.
#[allow(dead_code)]
pub fn score_v2(
    path: &str,
    size: u64,
    cache_domain: CacheDomain,
    ownership: Ownership,
    open_files: Option<&HashSet<PathBuf>>,
    history_cleaned: Option<&HashSet<String>>,
) -> ClassifiedFile {
    score_v3(&RiskSignals {
        path,
        size,
        domain: &cache_domain,
        ownership: &ownership,
        open_files,
        history_cleaned,
        memory_modifier: 0.0,
        age_days: file_age_days(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_safe_browser_cache() {
        let s = RiskSignals {
            path: "/home/user/.cache/mozilla/firefox/cache2/entry",
            size: 1024,
            domain: &CacheDomain::Browser,
            ownership: &Ownership::User { uid: 1000 },
            open_files: None,
            history_cleaned: None,
            memory_modifier: 0.0,
            age_days: Some(120.0),
        };
        let r = score_v3(&s);
        assert!(matches!(r.decision, Decision::Safe));
        assert!(r.risk_score < 0.12);
    }

    #[test]
    fn test_open_file_is_high_risk() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("open.txt");
        fs::write(&f, b"test").unwrap();

        let mut open = HashSet::new();
        open.insert(f.clone());

        let s = RiskSignals {
            path: &f.to_string_lossy(),
            size: 4,
            domain: &CacheDomain::Browser,
            ownership: &Ownership::User { uid: 1000 },
            open_files: Some(&open),
            history_cleaned: None,
            memory_modifier: 0.0,
            age_days: Some(0.0),
        };
        let r = score_v3(&s);
        assert!(r.risk_score > 0.5);
    }

    #[test]
    fn test_system_unknown_is_protected() {
        let s = RiskSignals {
            path: "/usr/local/weird/cache",
            size: 100,
            domain: &CacheDomain::Unknown,
            ownership: &Ownership::System,
            open_files: None,
            history_cleaned: None,
            memory_modifier: 0.0,
            age_days: Some(50.0),
        };
        let r = score_v3(&s);
        assert!(matches!(r.decision, Decision::Protected));
    }

    #[test]
    fn test_different_domains_get_different_scores() {
        let browser = score_v3(&RiskSignals {
            path: "/tmp/b",
            size: 100,
            domain: &CacheDomain::Browser,
            ownership: &Ownership::User { uid: 1000 },
            open_files: None,
            history_cleaned: None,
            memory_modifier: 0.0,
            age_days: Some(100.0),
        });
        let user_data = score_v3(&RiskSignals {
            path: "/tmp/u",
            size: 100,
            domain: &CacheDomain::UserData,
            ownership: &Ownership::User { uid: 1000 },
            open_files: None,
            history_cleaned: None,
            memory_modifier: 0.0,
            age_days: Some(100.0),
        });
        // Different domains should produce different scores
        assert_ne!(browser.risk_score, user_data.risk_score);
    }
}
