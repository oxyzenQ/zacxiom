// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Safe clean executor — v10: trash-based recovery.
//!
//! Executes deletions based on simulation results.
//! Only cleans files marked as cleanable at the given safety level.
//! Files are moved to trash before recording — undo can restore them.
//! Every action is logged (H3).

use crate::rules::ClassifiedFile;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Result of a clean operation.
#[derive(Debug)]
pub struct CleanReport {
    pub files_removed: usize,
    pub bytes_freed: u64,
    pub files_skipped: usize,
    pub bytes_skipped: u64,
    pub trash_paths: Vec<(String, String)>, // (original_path, trash_path)
    pub errors: Vec<CleanError>,
    /// Categorized error counts for summary display.
    pub error_counts: HashMap<String, usize>,
}

#[derive(Debug)]
pub struct CleanError {
    pub path: String,
    pub error: String,
}

/// Execute safe clean — moves files to trash directory for recoverable deletion.
///
/// Files are moved to `trash_dir` preserving their relative path structure.
/// Snapshot records the trash paths so `undo` can restore them.
///
/// - `smart`: also clean LowRisk files
/// - `force`: also clean Moderate files (after confirmation is handled by CLI)
/// - Protected files are NEVER cleaned regardless of flags.
pub fn clean(files: &[ClassifiedFile], smart: bool, force: bool, trash_dir: &Path) -> CleanReport {
    let mut report = CleanReport {
        files_removed: 0,
        bytes_freed: 0,
        files_skipped: 0,
        bytes_skipped: 0,
        trash_paths: Vec::new(),
        errors: Vec::new(),
        error_counts: HashMap::new(),
    };

    // Ensure trash directory exists
    if let Err(e) = fs::create_dir_all(trash_dir) {
        report.errors.push(CleanError {
            path: trash_dir.to_string_lossy().to_string(),
            error: format!("Cannot create trash directory: {e}"),
        });
        // Skip all files if we can't create trash
        for file in files {
            if file.decision.is_cleanable(smart, force) {
                report.files_skipped += 1;
                report.bytes_skipped += file.size;
            }
        }
        return report;
    }

    for file in files {
        if file.decision.is_cleanable(smart, force) {
            let src = Path::new(&file.path);
            // Build trash path preserving filename uniqueness
            let trash_path = build_trash_path(trash_dir, &file.path);

            // Ensure parent directory exists in trash
            if let Some(parent) = trash_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            // Try rename first (fast, same filesystem), fall back to copy+remove
            match fs::rename(src, &trash_path) {
                Ok(()) => {
                    report.files_removed += 1;
                    report.bytes_freed += file.size;
                    report
                        .trash_paths
                        .push((file.path.clone(), trash_path.to_string_lossy().to_string()));
                }
                Err(_e) => {
                    // Cross-filesystem: try copy + remove
                    match fs::copy(src, &trash_path) {
                        Ok(_) => {
                            match fs::remove_file(src) {
                                Ok(()) => {
                                    report.files_removed += 1;
                                    report.bytes_freed += file.size;
                                    report.trash_paths.push((
                                        file.path.clone(),
                                        trash_path.to_string_lossy().to_string(),
                                    ));
                                }
                                Err(rm_err) => {
                                    // Copied to trash but can't remove original.
                                    // Clean up the trash copy — don't leave duplicates.
                                    let _ = fs::remove_file(&trash_path);
                                    let cat = categorize_error(&rm_err.to_string());
                                    report
                                        .error_counts
                                        .entry(cat.clone())
                                        .and_modify(|c| *c += 1)
                                        .or_insert(1);
                                    report.errors.push(CleanError {
                                        path: file.path.clone(),
                                        error: format!(
                                            "Skipped ({cat}): file preserved at original location"
                                        ),
                                    });
                                }
                            }
                        }
                        Err(cp_err) => {
                            let cat = categorize_error(&cp_err.to_string());
                            report
                                .error_counts
                                .entry(cat.clone())
                                .and_modify(|c| *c += 1)
                                .or_insert(1);
                            report.errors.push(CleanError {
                                path: file.path.clone(),
                                error: format!("Skipped ({cat}): {cp_err}"),
                            });
                        }
                    }
                }
            }
        } else {
            report.files_skipped += 1;
            report.bytes_skipped += file.size;
        }
    }

    report
}

/// Build a unique trash path for a file, preserving directory structure.
fn build_trash_path(trash_dir: &Path, original_path: &str) -> PathBuf {
    // Use a sanitized version of the original path to avoid collisions
    let sanitized = original_path
        .replace('/', "_")
        .trim_start_matches('_')
        .to_string();
    trash_dir.join(sanitized)
}

/// Categorize an OS error string into a user-friendly label.
fn categorize_error(err_msg: &str) -> String {
    let lower = err_msg.to_lowercase();
    if lower.contains("permission") {
        "Permission denied".into()
    } else if lower.contains("read-only") {
        "Read-only filesystem".into()
    } else if lower.contains("not found") || lower.contains("no such file") {
        "Already removed".into()
    } else if lower.contains("broken") || lower.contains("symlink") {
        "Broken symlink".into()
    } else if lower.contains("text file busy") || lower.contains("in use") {
        "File in use".into()
    } else {
        "Unknown".into()
    }
}

/// Format a clean report for display with categorized error summary.
pub fn format_clean_report(report: &CleanReport) -> String {
    let mut out = String::new();

    out.push_str("CLEAN REPORT\n");
    out.push_str("────────────\n\n");

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
        // Show categorized summary first
        if !report.error_counts.is_empty() {
            let mut sorted: Vec<_> = report.error_counts.iter().collect();
            sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            for (cat, count) in &sorted {
                out.push_str(&format!("    {cat}: {count}\n"));
            }
            out.push('\n');
        }
        // Then show first 5 individual errors
        for err in report.errors.iter().take(5) {
            out.push_str(&format!("    {} → {}\n", err.path, err.error));
        }
        if report.errors.len() > 5 {
            out.push_str(&format!(
                "    ... and {} more errors\n",
                report.errors.len() - 5
            ));
        }
    }

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
            engine_category: String::new(),
            engine_confidence: 0,
        }
    }

    #[test]
    fn test_clean_safe_only() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("safe_file.txt");
        fs::write(&file_path, b"hello world").unwrap();
        let trash = tmp.path().join("trash");

        let files = vec![make_file(
            file_path.to_string_lossy().as_ref(),
            11,
            Decision::Safe,
        )];

        let report = clean(&files, false, false, &trash);
        assert_eq!(report.files_removed, 1);
        assert_eq!(report.bytes_freed, 11);
        assert!(!file_path.exists());
        assert!(!report.trash_paths.is_empty());
        // File should exist in trash
        assert_eq!(report.trash_paths.len(), 1);
    }

    #[test]
    fn test_clean_skips_low_risk_without_smart() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("low_risk.txt");
        fs::write(&file_path, b"data").unwrap();
        let trash = tmp.path().join("trash");

        let files = vec![make_file(
            file_path.to_string_lossy().as_ref(),
            4,
            Decision::LowRisk,
        )];

        let report = clean(&files, false, false, &trash);
        assert_eq!(report.files_skipped, 1);
        assert!(file_path.exists()); // still there
    }

    #[test]
    fn test_clean_never_removes_protected() {
        let tmp = TempDir::new().unwrap();
        let trash = tmp.path().join("trash");
        let files = vec![make_file("/etc/fake", 100, Decision::Protected)];
        let report = clean(&files, true, true, &trash);
        assert_eq!(report.files_skipped, 1);
        assert_eq!(report.files_removed, 0);
    }

    #[test]
    fn test_trash_path_is_recorded() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("recoverable.txt");
        fs::write(&file_path, b"precious data").unwrap();
        let trash = tmp.path().join("trash");

        let files = vec![make_file(
            file_path.to_string_lossy().as_ref(),
            13,
            Decision::Safe,
        )];

        let report = clean(&files, false, false, &trash);
        assert_eq!(report.files_removed, 1);
        assert_eq!(report.trash_paths.len(), 1);
        let (orig, trash_path) = &report.trash_paths[0];
        assert_eq!(orig, &file_path.to_string_lossy().to_string());
        // Trash file should exist
        assert!(Path::new(trash_path).exists());
    }
}
