// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Production safety lock — hard guarantees enforced at runtime.
//!
//! Validates that every operation meets the H-rule requirements
//! before execution. Acts as the final gate before any file mutation.

use crate::rules::{ClassifiedFile, Decision};

/// Safety lock validation result.
pub struct SafetyCheck {
    pub passed: bool,
    pub violations: Vec<String>,
}

/// Run all safety checks before a clean operation.
pub fn validate_clean(
    files: &[ClassifiedFile],
    smart: bool,
    force: bool,
    simulation_shown: bool,
) -> SafetyCheck {
    let mut violations = Vec::new();

    // H1: No silent deletion — simulation must be shown
    if !simulation_shown {
        violations.push("H1 VIOLATION: Simulation report was not displayed".into());
    }

    // H3: Every action must be logged — files must have reasons
    for f in files {
        if f.risk_reasons.is_empty() {
            violations.push(format!("H3 VIOLATION: No reason for {}", f.path));
        }
    }

    // H6: Force mode requires explicit confirmation
    if force && !simulation_shown {
        violations.push("H6 VIOLATION: --force without simulation display".into());
    }

    // Check no protected files are being cleaned
    for f in files {
        if matches!(f.decision, Decision::Protected) && f.decision.is_cleanable(smart, force) {
            violations.push(format!(
                "H2 VIOLATION: Protected file {} would be cleaned",
                f.path
            ));
        }
    }

    // Check HighRisk files aren't being cleaned without force + 2nd confirm
    if !force {
        for f in files {
            if matches!(f.decision, Decision::HighRisk) && f.decision.is_cleanable(smart, force) {
                violations.push(format!(
                    "H4/H6 VIOLATION: HighRisk file {} would be cleaned without force",
                    f.path
                ));
            }
        }
    }

    SafetyCheck {
        passed: violations.is_empty(),
        violations,
    }
}

/// Production readiness check for the entire system.
pub fn system_health_check() -> SafetyCheck {
    let mut violations = Vec::new();

    // Verify the binary can access its own version
    if option_env!("CARGO_PKG_VERSION").is_none() {
        violations.push("BUILD: Version not embedded".into());
    }

    // Verify snapshot directory is writable
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    let cache = std::path::PathBuf::from(home).join(".cache/zacxiom");
    if let Err(e) = std::fs::create_dir_all(&cache) {
        violations.push(format!("IO: Cannot write to cache dir: {e}"));
    }

    SafetyCheck {
        passed: violations.is_empty(),
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, Ownership};

    fn make_file(path: &str, decision: Decision) -> ClassifiedFile {
        ClassifiedFile {
            path: path.into(),
            size: 100,
            cache_domain: CacheDomain::Browser,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.0,
            risk_reasons: vec!["test reason".into()],
            decision,
            engine_category: String::new(),
            engine_confidence: 0,
        }
    }

    #[test]
    fn test_safety_check_passes() {
        let files = vec![make_file("/tmp/safe", Decision::Safe)];
        let check = validate_clean(&files, false, false, true);
        assert!(check.passed);
    }

    #[test]
    fn test_safety_check_fails_without_simulation() {
        let files = vec![make_file("/tmp/safe", Decision::Safe)];
        let check = validate_clean(&files, false, false, false);
        assert!(!check.passed);
    }

    #[test]
    fn test_safety_check_blocks_protected() {
        let files = vec![make_file("/etc/passwd", Decision::Protected)];
        let check = validate_clean(&files, true, true, true);
        assert!(check.passed); // Not cleanable even with force
    }

    #[test]
    fn test_system_health_check() {
        let check = system_health_check();
        assert!(check.passed);
    }
}
