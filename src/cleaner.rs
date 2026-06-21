// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Safe clean executor.
//!
//! Executes deletions based on simulation results.
//! Only cleans files marked as cleanable at the given safety level.
//! Every action is logged (H3).

use crate::rules::ClassifiedFile;
use std::fs;
use std::path::Path;

/// Result of a clean operation.
#[derive(Debug)]
pub struct CleanReport {
    pub files_removed: usize,
    pub bytes_freed: u64,
    pub files_skipped: usize,
    pub bytes_skipped: u64,
    pub errors: Vec<CleanError>,
}

#[derive(Debug)]
pub struct CleanError {
    pub path: String,
    pub error: String,
}

/// Execute safe clean on classified files.
///
/// - `smart`: also clean LowRisk files
/// - `force`: also clean Moderate files (after confirmation is handled by CLI)
/// - Protected files are NEVER cleaned regardless of flags.
pub fn clean(files: &[ClassifiedFile], smart: bool, force: bool) -> CleanReport {
    let mut report = CleanReport {
        files_removed: 0,
        bytes_freed: 0,
        files_skipped: 0,
        bytes_skipped: 0,
        errors: Vec::new(),
    };

    for file in files {
        if file.decision.is_cleanable(smart, force) {
            match fs::remove_file(Path::new(&file.path)) {
                Ok(()) => {
                    report.files_removed += 1;
                    report.bytes_freed += file.size;
                }
                Err(e) => {
                    report.errors.push(CleanError {
                        path: file.path.clone(),
                        error: e.to_string(),
                    });
                }
            }
        } else {
            report.files_skipped += 1;
            report.bytes_skipped += file.size;
        }
    }

    report
}

/// Format a clean report for display.
pub fn format_clean_report(report: &CleanReport) -> String {
    let mut out = String::new();

    out.push_str("═══════════════════════════════════════════\n");
    out.push_str("  ZACXIOM CLEAN REPORT\n");
    out.push_str("═══════════════════════════════════════════\n\n");

    out.push_str(&format!(
        "  Files removed : {} ({})\n",
        report.files_removed,
        crate::simulator::human_size(report.bytes_freed)
    ));
    out.push_str(&format!(
        "  Files skipped : {} ({})\n",
        report.files_skipped,
        crate::simulator::human_size(report.bytes_skipped)
    ));

    if !report.errors.is_empty() {
        out.push_str(&format!("\n  Errors: {}\n", report.errors.len()));
        for err in &report.errors {
            out.push_str(&format!("    {} → {}\n", err.path, err.error));
        }
    }

    out.push_str("═══════════════════════════════════════════\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, Decision, Ownership};
    use std::fs;
    use tempfile::TempDir;

    fn make_file(path: &str, size: u64, decision: Decision) -> ClassifiedFile {
        ClassifiedFile {
            path: path.to_string(),
            size,
            cache_domain: CacheDomain::Browser,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.0,
            risk_reasons: vec!["test".into()],
            decision,
        }
    }

    #[test]
    fn test_clean_safe_only() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("safe_file.txt");
        fs::write(&file_path, b"hello world").unwrap();

        let files = vec![make_file(
            file_path.to_string_lossy().as_ref(),
            11,
            Decision::Safe,
        )];

        let report = clean(&files, false, false);
        assert_eq!(report.files_removed, 1);
        assert_eq!(report.bytes_freed, 11);
        assert!(!file_path.exists());
    }

    #[test]
    fn test_clean_skips_low_risk_without_smart() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("low_risk.txt");
        fs::write(&file_path, b"data").unwrap();

        let files = vec![make_file(
            file_path.to_string_lossy().as_ref(),
            4,
            Decision::LowRisk,
        )];

        let report = clean(&files, false, false);
        assert_eq!(report.files_skipped, 1);
        assert!(file_path.exists()); // still there
    }

    #[test]
    fn test_clean_never_removes_protected() {
        let files = vec![make_file("/etc/fake", 100, Decision::Protected)];
        let report = clean(&files, true, true);
        assert_eq!(report.files_skipped, 1);
        assert_eq!(report.files_removed, 0);
    }
}
