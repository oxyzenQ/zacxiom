// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Rule-based risk scoring engine v2.
//!
//! v2 adds process-aware protection: files open by running processes
//! are flagged as HighRisk regardless of cache domain.
//! History-aware: files previously cleaned by user are slightly lower risk.

use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};
use std::collections::HashSet;
use std::path::PathBuf;

/// Score a classified file (v1 fallback).
#[allow(dead_code)]
pub fn score(
    path: &str,
    size: u64,
    cache_domain: CacheDomain,
    ownership: Ownership,
) -> ClassifiedFile {
    let (risk_score, risk_reasons, decision) =
        evaluate_inner(path, size, &cache_domain, &ownership, None, None);

    ClassifiedFile {
        path: path.to_string(),
        size,
        cache_domain,
        ownership,
        risk_score,
        risk_reasons,
        decision,
    }
}

/// Score with process awareness + history context (v2 engine).
pub fn score_v2(
    path: &str,
    size: u64,
    cache_domain: CacheDomain,
    ownership: Ownership,
    open_files: Option<&HashSet<PathBuf>>,
    history_cleaned: Option<&HashSet<String>>,
) -> ClassifiedFile {
    let (risk_score, risk_reasons, decision) = evaluate_inner(
        path,
        size,
        &cache_domain,
        &ownership,
        open_files,
        history_cleaned,
    );

    ClassifiedFile {
        path: path.to_string(),
        size,
        cache_domain,
        ownership,
        risk_score,
        risk_reasons,
        decision,
    }
}

fn evaluate_inner(
    path: &str,
    _size: u64,
    domain: &CacheDomain,
    ownership: &Ownership,
    open_files: Option<&HashSet<PathBuf>>,
    history_cleaned: Option<&HashSet<String>>,
) -> (f64, Vec<String>, Decision) {
    let path_buf = PathBuf::from(path);

    // R5: Protected paths — hard block
    if matches!(ownership, Ownership::System) && matches!(domain, CacheDomain::Unknown) {
        return (
            1.0,
            vec!["System-owned file with unknown cache domain".into()],
            Decision::Protected,
        );
    }

    // v2: Process-aware check — file open by running process → HighRisk
    if let Some(open) = open_files {
        // Check both the raw path and canonicalized
        let resolved = if path_buf.exists() {
            path_buf.canonicalize().unwrap_or_else(|_| path_buf.clone())
        } else {
            path_buf.clone()
        };

        if open.contains(&path_buf) || open.contains(&resolved) {
            return (
                0.85,
                vec!["File is currently open by a running process".into()],
                Decision::HighRisk,
            );
        }
    }

    // v2: History-aware — previously cleaned files are slightly lower risk
    let history_bonus = if let Some(h) = history_cleaned {
        if h.contains(path) {
            -0.05
        } else {
            0.0
        }
    } else {
        0.0
    };

    // R1: Safe = known cache + user-owned or orphan
    if matches!(
        domain,
        CacheDomain::Browser
            | CacheDomain::BuildArtifact
            | CacheDomain::Developer
            | CacheDomain::PackageManager
    ) && matches!(ownership, Ownership::User { .. } | Ownership::Orphan)
    {
        let score = (0.0_f64).max(0.0 + history_bonus);
        return (
            score,
            vec!["User-owned known cache data".into()],
            Decision::Safe,
        );
    }

    // R2: Low Risk
    if matches!(domain, CacheDomain::System) {
        return (
            0.25 + history_bonus,
            vec!["System cache directory, low risk of impact".into()],
            Decision::LowRisk,
        );
    }
    if matches!(ownership, Ownership::Orphan) && matches!(domain, CacheDomain::UserData) {
        return (
            0.3 + history_bonus,
            vec!["Orphan user cache data".into()],
            Decision::LowRisk,
        );
    }

    if matches!(ownership, Ownership::Package { .. })
        && matches!(
            domain,
            CacheDomain::BuildArtifact | CacheDomain::Developer | CacheDomain::PackageManager
        )
    {
        return (
            (0.35_f64).max(0.35 + history_bonus),
            vec!["Package-owned but classified as safe cache".into()],
            Decision::LowRisk,
        );
    }

    // R3: Moderate
    if matches!(domain, CacheDomain::Unknown) && matches!(ownership, Ownership::User { .. }) {
        return (
            (0.55_f64).max(0.55 + history_bonus),
            vec!["Unknown cache type, user-owned — moderate caution".into()],
            Decision::Moderate,
        );
    }

    // R4: High Risk fallback
    (
        (0.75_f64).max(0.75 + history_bonus),
        vec!["Unclassified risk profile — requires careful review".into()],
        Decision::HighRisk,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_cache_is_safe() {
        let result = score(
            "/home/user/.cache/mozilla/firefox/cache2/entry",
            1024,
            CacheDomain::Browser,
            Ownership::User { uid: 1000 },
        );
        assert!(matches!(result.decision, Decision::Safe));
    }

    #[test]
    fn test_system_cache_is_low_risk() {
        let result = score(
            "/var/cache/man/page.db",
            512,
            CacheDomain::System,
            Ownership::Orphan,
        );
        assert!(matches!(result.decision, Decision::LowRisk));
    }

    #[test]
    fn test_unknown_domain_is_moderate() {
        let result = score(
            "/home/user/some/file.bin",
            2048,
            CacheDomain::Unknown,
            Ownership::User { uid: 1000 },
        );
        assert!(matches!(result.decision, Decision::Moderate));
    }

    #[test]
    fn test_system_unknown_is_protected() {
        let result = score(
            "/usr/local/weird/file",
            4096,
            CacheDomain::Unknown,
            Ownership::System,
        );
        assert!(matches!(result.decision, Decision::Protected));
    }

    #[test]
    fn test_v2_history_bonus_reduces_risk() {
        let mut history = HashSet::new();
        history.insert("/home/user/old_cache/file.bin".to_string());

        let result = score_v2(
            "/home/user/old_cache/file.bin",
            100,
            CacheDomain::UserData,
            Ownership::Orphan,
            None,
            Some(&history),
        );
        // Risk should be slightly reduced due to history
        assert!(result.risk_score < 0.35);
    }

    #[test]
    fn test_v2_open_file_is_high_risk() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"test").unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let mut open = HashSet::new();
        open.insert(tmp.path().to_path_buf());

        let result = score_v2(
            &path,
            4,
            CacheDomain::Browser,
            Ownership::User { uid: 1000 },
            Some(&open),
            None,
        );
        assert!(matches!(result.decision, Decision::HighRisk));
        assert!(result.risk_score > 0.8);
    }
}
