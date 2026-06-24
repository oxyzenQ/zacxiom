// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Domain summary engine — aggregates files by cache domain.
//!
//! Users care about categories and decisions, not individual file listings.
//! This module produces domain-level summaries from classified files.

use crate::rules::{CacheDomain, ClassifiedFile, Decision};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct DomainSummary {
    pub domain: String,
    pub file_count: usize,
    pub total_size: u64,
    pub risk_label: String,
    pub risk_score: f64,
    pub dominant_decision: String,
    pub safe_count: usize,
    pub blocked_count: usize,
    pub status: DomainStatus,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum DomainStatus {
    Safe,
    Reclaimable,
    InUse,
    Blocked,
    Mixed,
}

impl DomainStatus {
    pub fn label(&self) -> &'static str {
        match self {
            DomainStatus::Safe => "SAFE",
            DomainStatus::Reclaimable => "RECLAIMABLE",
            DomainStatus::InUse => "IN USE",
            DomainStatus::Blocked => "BLOCKED",
            DomainStatus::Mixed => "MIXED",
        }
    }
}

/// Aggregate classified files into domain summaries.
pub fn summarize(files: &[ClassifiedFile]) -> Vec<DomainSummary> {
    let mut domains: HashMap<String, Vec<&ClassifiedFile>> = HashMap::new();

    for f in files {
        let key = domain_key(&f.cache_domain, &f.path);
        domains.entry(key).or_default().push(f);
    }

    let mut summaries: Vec<DomainSummary> = domains
        .into_iter()
        .map(|(domain, entries)| {
            let file_count = entries.len();
            let total_size: u64 = entries.iter().map(|e| e.size).sum();
            let avg_risk: f64 =
                entries.iter().map(|e| e.risk_score).sum::<f64>() / file_count.max(1) as f64;

            let safe_count = entries
                .iter()
                .filter(|e| matches!(e.decision, Decision::Safe))
                .count();
            let low_risk_count = entries
                .iter()
                .filter(|e| matches!(e.decision, Decision::LowRisk))
                .count();
            let blocked_count = entries
                .iter()
                .filter(|e| matches!(e.decision, Decision::HighRisk | Decision::Protected))
                .count();

            let dominant_decision = if safe_count > file_count / 2 {
                "SAFE"
            } else if blocked_count > file_count / 2 {
                "BLOCKED"
            } else if low_risk_count > file_count / 2 {
                "LOWRISK"
            } else {
                "MIXED"
            };

            // v6.4.0: risk_label must reflect the dominant decision, not just
            // the raw average risk score. The bridge override changes toolchain
            // files from Safe → LowRisk, but their risk_score remains near 0,
            // so avg_risk < 0.15 → "SAFE" even though dominant_decision is "LOWRISK".
            // Use dominant_decision as the primary signal, with risk_score as refinement.
            let risk_label = match dominant_decision {
                "SAFE" => "SAFE",
                "BLOCKED" => "BLOCKED",
                "LOWRISK" => "LOWRISK",
                _ => {
                    // Mixed — fall back to risk score for granularity
                    if avg_risk < 0.15 {
                        "SAFE"
                    } else if avg_risk < 0.35 {
                        "LOW"
                    } else if avg_risk < 0.6 {
                        "MEDIUM"
                    } else if avg_risk < 0.85 {
                        "HIGH"
                    } else {
                        "CRITICAL"
                    }
                }
            };

            let status = if blocked_count > file_count / 2 {
                DomainStatus::Blocked
            } else if safe_count > file_count / 2 {
                DomainStatus::Safe
            } else if low_risk_count > file_count / 2 {
                // v6.4.0: LowRisk-dominant domains are "Reclaimable" —
                // they require --smart but are still cleanable.
                DomainStatus::Reclaimable
            } else if safe_count > 0 || low_risk_count > 0 {
                DomainStatus::Reclaimable
            } else if entries
                .iter()
                .any(|e| matches!(e.decision, Decision::HighRisk))
            {
                DomainStatus::InUse
            } else {
                DomainStatus::Mixed
            };

            DomainSummary {
                domain,
                file_count,
                total_size,
                risk_label: risk_label.to_string(),
                risk_score: avg_risk,
                dominant_decision: dominant_decision.to_string(),
                safe_count,
                blocked_count,
                status,
            }
        })
        .collect();

    // Sort by size descending
    summaries.sort_by_key(|b| std::cmp::Reverse(b.total_size));
    summaries
}

fn domain_key(domain: &CacheDomain, path: &str) -> String {
    let base = match domain {
        CacheDomain::Browser => {
            if path.contains("firefox") || path.contains("mozilla") {
                "Firefox Browser Cache"
            } else if path.contains("chromium") || path.contains("chrome") {
                "Chromium Browser Cache"
            } else {
                "Browser Cache"
            }
        }
        CacheDomain::BuildArtifact => {
            if path.contains("target/") {
                "Rust Build Cache"
            } else if path.contains("node_modules") {
                "Node.js Modules"
            } else if path.contains("__pycache__") {
                "Python Bytecode Cache"
            } else if path.contains(".gradle") || path.contains("build/") {
                "Java/Gradle Build Cache"
            } else if path.contains(".next/") || path.contains("dist/") {
                "Web Build Artifacts"
            } else {
                "Build Artifacts"
            }
        }
        CacheDomain::System => {
            if path.contains("mesa") || path.contains("shader") {
                "Mesa Shader Cache"
            } else if path.contains("/tmp/") {
                "Temporary Files"
            } else if path.contains("dxvk") || path.contains("vkd3d") {
                "DXVK/VKD3D Shader Cache"
            } else if path.contains("steam") && path.contains("shadercache") {
                "Steam Shader Cache"
            } else {
                "System Cache"
            }
        }
        CacheDomain::PackageManager => "Package Manager Cache",
        CacheDomain::Developer => {
            if path.contains(".cargo") {
                "Cargo Registry & Build Cache"
            } else if path.contains("rustup") {
                "Rustup Toolchains"
            } else if path.contains(".npm") || path.contains("yarn") || path.contains("pnpm") {
                "JavaScript Package Cache"
            } else if path.contains("pip") || path.contains("uv") {
                "Python Package Cache"
            } else if path.contains(".m2") {
                "Maven Repository Cache"
            } else if path.contains("docker") || path.contains("containers") {
                "Docker/Container Cache"
            } else if path.contains("huggingface")
                || path.contains("ollama")
                || path.contains("torch")
                || path.contains("modelscope")
            {
                "AI/ML Model Cache"
            } else {
                "Developer Cache"
            }
        }
        CacheDomain::UserData => {
            if path.contains("trash") {
                "Desktop Trash"
            } else if path.contains("compatdata") {
                "Proton/Steam Compat Data"
            } else if path.contains("lutris") || path.contains("heroic") {
                "Gaming Runner Cache"
            } else if path.contains("thumbnails") {
                "Thumbnail Cache"
            } else if path.contains("thunderbird") {
                "Email Client Cache"
            } else {
                "User Cache"
            }
        }
        CacheDomain::Unknown => {
            if path.contains("mesa") || path.contains("shader") {
                "Mesa Shader Cache"
            } else if path.contains("downloads") {
                "Downloads Directory"
            } else {
                "Application Cache"
            }
        }
    };
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};

    fn cf(
        path: &str,
        size: u64,
        domain: CacheDomain,
        decision: Decision,
        risk: f64,
    ) -> ClassifiedFile {
        ClassifiedFile {
            path: path.into(),
            size,
            cache_domain: domain,
            ownership: Ownership::User { uid: 1000 },
            risk_score: risk,
            risk_reasons: vec!["test".into()],
            decision,
            engine_category: String::new(),
            engine_confidence: 0,
        }
    }

    #[test]
    fn test_summarize_groups_by_domain() {
        let files = vec![
            cf(
                "/tmp/cache/a",
                100,
                CacheDomain::Browser,
                Decision::Safe,
                0.05,
            ),
            cf(
                "/tmp/cache/b",
                200,
                CacheDomain::Browser,
                Decision::Safe,
                0.10,
            ),
            cf(
                "/tmp/cache/c",
                500,
                CacheDomain::System,
                Decision::HighRisk,
                0.90,
            ),
        ];
        let summaries = summarize(&files);
        assert_eq!(summaries.len(), 2);
        let browser = summaries
            .iter()
            .find(|s| s.domain.contains("Browser"))
            .unwrap();
        assert_eq!(browser.file_count, 2);
        assert_eq!(browser.total_size, 300);
        assert_eq!(browser.status, DomainStatus::Safe);
    }

    // ═══════════════════════════════════════════════════════════
    // v6.4.0: Domain label must reflect dominant decision
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_lowrisk_domain_label_is_lowrisk_not_safe() {
        // Toolchain files: LowRisk decision but near-zero risk score.
        // Domain label must say "LOWRISK", not "SAFE".
        let files = vec![
            cf(
                "/home/user/.rustup/toolchains/stable-x86_64/bin/rustc",
                1000,
                CacheDomain::Developer,
                Decision::LowRisk,
                0.01,
            ),
            cf(
                "/home/user/.rustup/toolchains/stable-x86_64/bin/cargo",
                800,
                CacheDomain::Developer,
                Decision::LowRisk,
                0.02,
            ),
            cf(
                "/home/user/.rustup/toolchains/stable-x86_64/lib/libstd.so",
                2000,
                CacheDomain::Developer,
                Decision::LowRisk,
                0.01,
            ),
        ];
        let summaries = summarize(&files);
        let rustup = summaries
            .iter()
            .find(|s| s.domain.contains("Rustup"))
            .unwrap();
        assert_eq!(rustup.risk_label, "LOWRISK");
        assert_eq!(rustup.dominant_decision, "LOWRISK");
        assert_eq!(rustup.status, DomainStatus::Reclaimable);
    }

    #[test]
    fn test_safe_domain_label_still_safe() {
        // Regular browser cache: Safe decision → label should be "SAFE"
        let files = vec![
            cf(
                "/home/user/.cache/mozilla/firefox/cache2/entry1",
                100,
                CacheDomain::Browser,
                Decision::Safe,
                0.05,
            ),
            cf(
                "/home/user/.cache/mozilla/firefox/cache2/entry2",
                200,
                CacheDomain::Browser,
                Decision::Safe,
                0.03,
            ),
        ];
        let summaries = summarize(&files);
        let browser = summaries
            .iter()
            .find(|s| s.domain.contains("Browser"))
            .unwrap();
        assert_eq!(browser.risk_label, "SAFE");
        assert_eq!(browser.dominant_decision, "SAFE");
    }

    #[test]
    fn test_mixed_domain_uses_risk_score_fallback() {
        // Mixed decisions → risk_label falls back to avg_risk score
        let files = vec![
            cf("/tmp/a", 100, CacheDomain::Unknown, Decision::Safe, 0.05),
            cf(
                "/tmp/b",
                100,
                CacheDomain::Unknown,
                Decision::Moderate,
                0.40,
            ),
        ];
        let summaries = summarize(&files);
        let unknown = summaries
            .iter()
            .find(|s| s.domain.contains("Application"))
            .unwrap();
        // 1 Safe, 1 Moderate — neither > half, so MIXED
        assert_eq!(unknown.dominant_decision, "MIXED");
        // avg_risk = 0.225 → "LOW"
        assert_eq!(unknown.risk_label, "LOW");
    }
}
