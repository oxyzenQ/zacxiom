// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Confidence scoring engine — numerical certainty layer.
//!
//! v6.3: Produces a 0-100 confidence score for every classification.
//! Built on top of the v6.2.5 engine architecture.
//! Informational only — does not change classification or risk decisions.

use super::types::{Category, ClassificationResult};
use std::path::Path;

/// Assign a confidence score (0-100) to a classification result.
pub fn score(result: &mut ClassificationResult, path: &Path, lower: &str) {
    let mut score: i32 = 50; // neutral starting point
    let mut reasons: Vec<String> = Vec::new();

    // ═══════════════════════════════════════════════════════════
    // Signal 1: Rule match strength
    // ═══════════════════════════════════════════════════════════
    let match_strength = if !result.matched_by.is_empty() && result.matched_by != "default" {
        let rule_name = &result.matched_by;
        if rule_name.starts_with("sys-") || rule_name.starts_with("sec-") {
            // System and security rules are highest confidence
            score += 40;
            reasons.push("✓ Exact system rule matched".into());
            40
        } else if rule_name.starts_with("config-") || rule_name.starts_with("user-") {
            score += 35;
            reasons.push("✓ Exact path rule matched".into());
            35
        } else if rule_name.starts_with("cache-") || rule_name.starts_with("app-") {
            score += 30;
            reasons.push("✓ Known cache/application rule matched".into());
            30
        } else {
            score += 20;
            reasons.push("✓ Rule matched".into());
            20
        }
    } else {
        score -= 30;
        reasons.push("✗ No classification rule matched".into());
        0
    };

    // ═══════════════════════════════════════════════════════════
    // Signal 2: Metadata confidence
    // ═══════════════════════════════════════════════════════════
    if super::metadata::is_elf_binary(path) {
        score += 15;
        reasons.push("✓ ELF binary detected".into());
    }
    if super::metadata::is_regular_executable(path) {
        score += 8;
        reasons.push("✓ Executable file detected".into());
    }
    if let Some(_size) = super::metadata::file_size(path) {
        // Known file size = we could read metadata
        score += 2;
    }

    // ═══════════════════════════════════════════════════════════
    // Signal 3: Regenerability confidence
    // ═══════════════════════════════════════════════════════════
    if result.regenerable {
        score += 20;
        reasons.push("✓ Regenerable content".into());
    } else if result.category.is_protected() {
        score += 20;
        reasons.push("✓ Known non-regenerable (protected)".into());
    }
    // Unknown regenerability: no bonus

    // ═══════════════════════════════════════════════════════════
    // Signal 4: Location confidence
    // ═══════════════════════════════════════════════════════════
    if lower.starts_with("/usr/")
        || lower.starts_with("/bin/")
        || lower.starts_with("/etc/")
        || lower.starts_with("/lib/")
    {
        score += 15;
        reasons.push("✓ Known system location".into());
    } else if lower.contains("/home/") || lower.contains("/root/") {
        if lower.contains("/.cache/")
            || lower.contains("/.cargo/")
            || lower.contains("/.npm/")
            || lower.contains("/.config/")
        {
            score += 12;
            reasons.push("✓ Known user cache/config location".into());
        } else if lower.contains("/Desktop")
            || lower.contains("/Documents")
            || lower.contains("/Music")
            || lower.contains("/Pictures")
        {
            score += 15;
            reasons.push("✓ Known user content location".into());
        } else {
            score += 5;
            reasons.push("✓ User home location".into());
        }
    }

    // ═══════════════════════════════════════════════════════════
    // Signal 5: Classification penalties
    // ═══════════════════════════════════════════════════════════
    if result.category == Category::Unknown {
        score -= 20;
        reasons.push("✗ Unknown category".into());
    }
    if match_strength == 0 && result.category != Category::Unknown {
        score -= 10;
        reasons.push("✗ Metadata-only classification".into());
    }

    // Clamp to 0-100
    result.confidence_score = score.clamp(0, 100) as u8;
    result.confidence_reasons = reasons;

    // Build explanation string
    result.confidence_explanation = confidence_label(result.confidence_score).to_string();
}

/// Human-readable label for a confidence score.
pub fn confidence_label(score: u8) -> &'static str {
    match score {
        90..=100 => "Very High Confidence",
        70..=89 => "High Confidence",
        50..=69 => "Moderate Confidence",
        30..=49 => "Low Confidence",
        _ => "Unknown / Needs Review",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::ClassificationResult;

    fn classify_and_score(path: &str) -> ClassificationResult {
        let p = Path::new(path);
        let mut result = crate::engine::classify(p);
        let lower = path.to_lowercase();
        score(&mut result, p, &lower);
        result
    }

    #[test]
    fn test_system_binary_high_confidence() {
        let r = classify_and_score("/usr/bin/bash");
        assert!(
            r.confidence_score >= 95,
            "expected >=95, got {}",
            r.confidence
        );
        assert!(r.confidence_explanation.contains("Very High"));
    }

    #[test]
    fn test_etc_config_high_confidence() {
        let r = classify_and_score("/etc/environment");
        assert!(
            r.confidence_score >= 95,
            "expected >=95, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_ssh_key_high_confidence() {
        let r = classify_and_score("/home/user/.ssh/id_ed25519");
        assert!(
            r.confidence_score >= 90,
            "expected >=90, got {}",
            r.confidence
        );
        assert!(r.category.is_protected());
    }

    #[test]
    fn test_cache_path_high_confidence() {
        let r = classify_and_score("/home/user/.cache/mozilla/firefox/cache2/entries/abc");
        // Browser cache should have high confidence
        assert!(
            r.confidence_score >= 80,
            "expected >=80, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_user_cache_moderate_high_confidence() {
        let r = classify_and_score("/home/user/.cache/some-app/data");
        assert!(
            r.confidence_score >= 60,
            "expected >=60, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_unknown_path_low_confidence() {
        let r = classify_and_score("/some/random/path/nowhere");
        assert!(
            r.confidence_score < 50,
            "expected <50, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_desktop_high_confidence() {
        let r = classify_and_score("/home/user/Desktop/report.pdf");
        assert!(
            r.confidence_score >= 70,
            "expected >=70, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_zshrc_high_confidence() {
        let r = classify_and_score("/home/user/.zshrc");
        assert!(
            r.confidence_score >= 70,
            "expected >=70, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_guess_binary_without_rule() {
        // A path that looks like a binary but isn't in a known system location
        let r = classify_and_score("/opt/some-app/bin/tool");
        assert!(
            r.confidence_score >= 90,
            "expected >=90, got {}",
            r.confidence
        );
    }

    #[test]
    fn test_confidence_range_valid() {
        let paths = [
            "/usr/bin/bash",
            "/etc/hosts",
            "/home/user/.ssh/id_rsa",
            "/home/user/.cache/app/data",
            "/tmp/random-file",
            "/home/user/.zshrc",
            "/mystery/path",
        ];
        for path in &paths {
            let r = classify_and_score(path);
            assert!(
                r.confidence_score <= 100,
                "path {}: confidence {} > 100",
                path,
                r.confidence
            );
            assert!(r.confidence_reasons.len() >= 1, "path {}: no reasons", path);
            assert!(
                !r.confidence_explanation.is_empty(),
                "path {}: no explanation",
                path
            );
        }
    }
}
