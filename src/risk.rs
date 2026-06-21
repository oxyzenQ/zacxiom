//! Rule-based risk scoring engine.
//!
//! Takes classified files and assigns risk scores (0.0–1.0) + decisions
//! based on immutable rules defined in RULES.md.

use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};

/// Score a classified file and return a complete ClassifiedFile with risk + decision.
pub fn score(
    path: &str,
    size: u64,
    cache_domain: CacheDomain,
    ownership: Ownership,
) -> ClassifiedFile {
    let (risk_score, risk_reasons, decision) = evaluate(path, size, &cache_domain, &ownership);

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

fn evaluate(
    _path: &str,
    _size: u64,
    domain: &CacheDomain,
    ownership: &Ownership,
) -> (f64, Vec<String>, Decision) {
    // R5: Protected paths are handled by scanner (they're skipped).
    // Here we check ownership-based protection.
    if matches!(ownership, Ownership::System) && matches!(domain, CacheDomain::Unknown) {
        return (
            1.0,
            vec!["System-owned file with unknown cache domain".into()],
            Decision::Protected,
        );
    }

    // R1: Safe = known cache + user-owned
    if matches!(
        domain,
        CacheDomain::Browser
            | CacheDomain::BuildArtifact
            | CacheDomain::Developer
            | CacheDomain::PackageManager
    ) && matches!(ownership, Ownership::User { .. } | Ownership::Orphan)
    {
        return (
            0.0,
            vec!["User-owned known cache data".into()],
            Decision::Safe,
        );
    }

    // R2: Low Risk = system cache or orphan
    if matches!(domain, CacheDomain::System) {
        return (
            0.25,
            vec!["System cache directory, low risk of impact".into()],
            Decision::LowRisk,
        );
    }
    if matches!(ownership, Ownership::Orphan) && matches!(domain, CacheDomain::UserData) {
        return (
            0.3,
            vec!["Orphan user cache data".into()],
            Decision::LowRisk,
        );
    }

    // R2: Package-owned cache
    if matches!(ownership, Ownership::Package { .. })
        && matches!(
            domain,
            CacheDomain::BuildArtifact | CacheDomain::Developer | CacheDomain::PackageManager
        )
    {
        return (
            0.35,
            vec!["Package-owned but classified as safe cache".into()],
            Decision::LowRisk,
        );
    }

    // R3: Moderate = unknown domain with user ownership
    if matches!(domain, CacheDomain::Unknown) && matches!(ownership, Ownership::User { .. }) {
        return (
            0.55,
            vec!["Unknown cache type, user-owned — moderate caution".into()],
            Decision::Moderate,
        );
    }

    // R4: High Risk = anything else not protected
    (
        0.75,
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
        assert_eq!(result.risk_score, 0.0);
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
}
