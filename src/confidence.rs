// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Confidence scoring — ★★★★★ tier system.
//!
//! Maps risk scores and domain classifications to user-facing
//! confidence tiers. Not "risk" — "how confident are we that this is safe?"

use crate::rules::{CacheDomain, ClassifiedFile, Decision};

/// Confidence tier for user-facing display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// ★★★★★ — Fully regenerable, zero data loss risk
    Maximum,
    /// ★★★★ — Regenerable but takes time
    High,
    /// ★★★ — Probably safe, quick review recommended
    Moderate,
    /// ★★ — May contain useful data, manual review needed
    Low,
    /// ★ — Potentially risky, not recommended
    Minimal,
    /// ⛔ — Never removable
    Protected,
}

impl Tier {
    pub fn stars(&self) -> String {
        let stars = match self {
            Tier::Maximum => "★★★★★",
            Tier::High => "★★★★",
            Tier::Moderate => "★★★",
            Tier::Low => "★★",
            Tier::Minimal => "★",
            Tier::Protected => "⛔",
        };
        crate::color::purple(stars)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Tier::Maximum => "Maximum Safety",
            Tier::High => "High Safety",
            Tier::Moderate => "Review Recommended",
            Tier::Low => "Caution",
            Tier::Minimal => "Manual Review Required",
            Tier::Protected => "Protected — Never Removable",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Tier::Maximum => "Fully regenerable. Zero data loss. Safe to clean anytime.",
            Tier::High => "Regenerable but may require time to rebuild. Safe with --smart.",
            Tier::Moderate => "Probably safe, but review before cleaning. Contains no user data.",
            Tier::Low => "May contain useful data. Manual review recommended before deleting.",
            Tier::Minimal => "Could be important. Review carefully. Not auto-cleaned.",
            Tier::Protected => "System-critical or user-sensitive. Never removed by Zacxiom.",
        }
    }

    /// How many clean modes include this tier?
    pub fn cleaned_by(&self) -> &'static str {
        match self {
            Tier::Maximum => "clean (default), --smart, --force",
            Tier::High => "--smart, --force",
            Tier::Moderate => "--force only",
            Tier::Low => "never auto-cleaned",
            Tier::Minimal => "never auto-cleaned",
            Tier::Protected => "never removed",
        }
    }
}

/// Compute confidence tier from a classified file.
///
/// v6.4.0: Engine-category-aware — semantic identity influences confidence.
/// A file classified as "Installed Toolchain" should not get ★★★★★ Maximum
/// even if its legacy decision is Safe and domain is Developer.
pub fn confidence(file: &ClassifiedFile) -> Tier {
    // Protected decisions always Protected
    if matches!(file.decision, Decision::Protected) {
        return Tier::Protected;
    }

    // v6.4.0: Engine category override — align identity with confidence.
    // Installed toolchains are not disposable cache even if legacy pipeline
    // scored them as Safe/Developer.
    if file.engine_category == "Toolchain Installation"
        || file.engine_category == "Toolchain Manager"
    {
        return match file.decision {
            Decision::Safe | Decision::LowRisk => Tier::High, // ★★★★ — requires --smart
            Decision::Moderate => Tier::Moderate,
            _ => Tier::Low,
        };
    }

    // Decision-based mapping with domain awareness
    match file.decision {
        Decision::Safe => Tier::Maximum,
        Decision::LowRisk => {
            // If it's a known regenerable domain, bump confidence
            match file.cache_domain {
                CacheDomain::Browser | CacheDomain::BuildArtifact | CacheDomain::Developer => {
                    Tier::Maximum
                }
                CacheDomain::PackageManager | CacheDomain::System => Tier::High,
                CacheDomain::UserData => Tier::Moderate,
                CacheDomain::Unknown => Tier::Moderate,
            }
        }
        Decision::Moderate => match file.cache_domain {
            CacheDomain::Browser | CacheDomain::BuildArtifact | CacheDomain::Developer => {
                Tier::High
            }
            _ => Tier::Moderate,
        },
        Decision::HighRisk => Tier::Low,
        Decision::Protected => Tier::Protected,
    }
}

/// Confidence summary for a set of files.
pub struct ConfidenceSummary {
    pub maximum: usize,
    pub high: usize,
    pub moderate: usize,
    pub low: usize,
    pub minimal: usize,
    pub protected: usize,
    pub total: usize,
}

impl ConfidenceSummary {
    /// Zeroed summary for test usage.
    pub fn empty() -> Self {
        Self {
            maximum: 0,
            high: 0,
            moderate: 0,
            low: 0,
            minimal: 0,
            protected: 0,
            total: 0,
        }
    }

    pub fn from_files(files: &[ClassifiedFile]) -> Self {
        let mut s = Self {
            maximum: 0,
            high: 0,
            moderate: 0,
            low: 0,
            minimal: 0,
            protected: 0,
            total: files.len(),
        };
        for f in files {
            match confidence(f) {
                Tier::Maximum => s.maximum += 1,
                Tier::High => s.high += 1,
                Tier::Moderate => s.moderate += 1,
                Tier::Low => s.low += 1,
                Tier::Minimal => s.minimal += 1,
                Tier::Protected => s.protected += 1,
            }
        }
        s
    }

    pub fn cleanable_default(&self) -> usize {
        self.maximum
    }

    pub fn cleanable_smart(&self) -> usize {
        self.maximum + self.high
    }

    pub fn cleanable_force(&self) -> usize {
        self.maximum + self.high + self.moderate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};

    fn file(domain: CacheDomain, decision: Decision) -> ClassifiedFile {
        ClassifiedFile {
            path: "/test/path".into(),
            size: 1000,
            cache_domain: domain,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.0,
            risk_reasons: vec![],
            decision,
            engine_category: String::new(),
            engine_confidence: 0,
        }
    }

    #[test]
    fn test_safe_browser_is_maximum() {
        let f = file(CacheDomain::Browser, Decision::Safe);
        assert_eq!(confidence(&f), Tier::Maximum);
    }

    #[test]
    fn test_lowrisk_browser_is_maximum() {
        let f = file(CacheDomain::Browser, Decision::LowRisk);
        assert_eq!(confidence(&f), Tier::Maximum);
    }

    #[test]
    fn test_moderate_unknown_is_moderate() {
        let f = file(CacheDomain::Unknown, Decision::Moderate);
        assert_eq!(confidence(&f), Tier::Moderate);
    }

    #[test]
    fn test_highrisk_is_low() {
        let f = file(CacheDomain::Browser, Decision::HighRisk);
        assert_eq!(confidence(&f), Tier::Low);
    }

    #[test]
    fn test_protected_is_protected() {
        let f = file(CacheDomain::Unknown, Decision::Protected);
        assert_eq!(confidence(&f), Tier::Protected);
    }

    #[test]
    fn test_lowrisk_userdata_is_moderate() {
        let f = file(CacheDomain::UserData, Decision::LowRisk);
        assert_eq!(confidence(&f), Tier::Moderate);
    }

    #[test]
    fn test_safe_buildartifact_maximum() {
        let f = file(CacheDomain::BuildArtifact, Decision::Safe);
        assert_eq!(confidence(&f), Tier::Maximum);
    }

    #[test]
    fn test_summary_counts() {
        let files = vec![
            file(CacheDomain::Browser, Decision::Safe),
            file(CacheDomain::Browser, Decision::Safe),
            file(CacheDomain::Developer, Decision::LowRisk),
            file(CacheDomain::Unknown, Decision::Moderate),
            file(CacheDomain::System, Decision::HighRisk),
            file(CacheDomain::Unknown, Decision::Protected),
        ];
        let s = ConfidenceSummary::from_files(&files);
        assert_eq!(s.total, 6);
        assert_eq!(s.maximum, 3); // 2 Safe Browser + 1 LowRisk Developer
        assert_eq!(s.moderate, 1);
        assert_eq!(s.low, 1);
        assert_eq!(s.protected, 1);
    }

    // ═══════════════════════════════════════════════════════════
    // v6.4.0: Toolchain policy tests
    // ═══════════════════════════════════════════════════════════

    fn file_with_engine(
        domain: CacheDomain,
        decision: Decision,
        engine_cat: &str,
    ) -> ClassifiedFile {
        ClassifiedFile {
            path: "/test/path".into(),
            size: 1000,
            cache_domain: domain,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.0,
            risk_reasons: vec![],
            decision,
            engine_category: engine_cat.to_string(),
            engine_confidence: 0,
        }
    }

    #[test]
    fn test_toolchain_installation_not_maximum() {
        // Toolchain installation should NOT get ★★★★★ Maximum
        let f = file_with_engine(
            CacheDomain::Developer,
            Decision::LowRisk,
            "Toolchain Installation",
        );
        assert_ne!(confidence(&f), Tier::Maximum);
        assert_eq!(confidence(&f), Tier::High); // ★★★★ — requires --smart
    }

    #[test]
    fn test_toolchain_manager_not_maximum() {
        let f = file_with_engine(CacheDomain::Developer, Decision::Safe, "Toolchain Manager");
        assert_ne!(confidence(&f), Tier::Maximum);
        assert_eq!(confidence(&f), Tier::High); // ★★★★ — requires --smart
    }

    #[test]
    fn test_toolchain_protected_stays_protected() {
        let f = file_with_engine(
            CacheDomain::System,
            Decision::Protected,
            "Toolchain Installation",
        );
        assert_eq!(confidence(&f), Tier::Protected);
    }

    #[test]
    fn test_regular_developer_cache_still_maximum() {
        // Regular developer cache (not toolchain) should still get ★★★★★
        let f = file_with_engine(CacheDomain::Developer, Decision::Safe, "Build Cache");
        assert_eq!(confidence(&f), Tier::Maximum);
    }
}
