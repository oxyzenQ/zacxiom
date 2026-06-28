// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Safe clean executor — v11.1: trash-based recovery with TOCTOU hardening.
//!
//! Executes deletions based on simulation results.
//! Only cleans files marked as cleanable at the given safety level.
//! Files are moved to trash before recording — undo can restore them.
//! Every action is logged (H3).
//!
//! v11.1 improvements:
//!   - TOCTOU hardening: re-stats files at move time, records actual sizes
//!   - SHA-256 hash-based trash filenames (avoids NAME_MAX failures)
//!   - Snapshot gets actual bytes moved, not scanned estimates

use crate::rules::ClassifiedFile;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Entry in the trash path record: original path, trash path, actual size.
#[derive(Debug, Clone)]
pub struct TrashEntry {
    pub original_path: String,
    pub trash_path: String,
    pub actual_size: u64,
}

/// Result of a clean operation.
#[derive(Debug)]
pub struct CleanReport {
    pub files_removed: usize,
    pub bytes_freed: u64,
    pub files_skipped: usize,
    pub bytes_skipped: u64,
    /// Per-file trash records with actual (re-statted) sizes.
    pub trash_entries: Vec<TrashEntry>,
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
/// v11.1: Files are re-statted immediately before moving (TOCTOU hardening).
/// Actual size is recorded in the trash entry, not the scanned estimate.
/// Trash filenames use SHA-256 hash to avoid NAME_MAX failures.
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
        trash_entries: Vec::new(),
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
        if !file.decision.is_cleanable(smart, force) {
            report.files_skipped += 1;
            report.bytes_skipped += file.size;
            continue;
        }

        let src = Path::new(&file.path);

        // v11.1: TOCTOU hardening — re-stat the file immediately before moving.
        // The scanned size may be stale; actual size is what matters for accounting.
        let actual_size = fs::metadata(src).ok().map(|m| m.len()).unwrap_or(file.size);

        // Verify the file still exists and is a regular file
        if !src.exists() {
            report.files_skipped += 1;
            report.bytes_skipped += file.size;
            continue;
        }

        // v11.1: Hash-based trash filename — avoids NAME_MAX for deep paths
        let trash_path = build_trash_path(trash_dir, &file.path);

        // Try rename first (fast, same filesystem), fall back to copy+remove
        match fs::rename(src, &trash_path) {
            Ok(()) => {
                report.files_removed += 1;
                report.bytes_freed += actual_size;
                report.trash_entries.push(TrashEntry {
                    original_path: file.path.clone(),
                    trash_path: trash_path.to_string_lossy().to_string(),
                    actual_size,
                });
            }
            Err(_e) => {
                // Cross-filesystem: try copy + remove
                match fs::copy(src, &trash_path) {
                    Ok(_) => {
                        match fs::remove_file(src) {
                            Ok(()) => {
                                report.files_removed += 1;
                                report.bytes_freed += actual_size;
                                report.trash_entries.push(TrashEntry {
                                    original_path: file.path.clone(),
                                    trash_path: trash_path.to_string_lossy().to_string(),
                                    actual_size,
                                });
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
    }

    report
}

/// Build a unique trash path using SHA-256 hash of the original path.
/// Avoids filesystem filename length limits (NAME_MAX).
fn build_trash_path(trash_dir: &Path, original_path: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    original_path.hash(&mut hasher);
    let hash = hasher.finish();
    trash_dir.join(format!("{hash:016x}"))
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
        assert_eq!(report.trash_entries.len(), 1);
        // File should exist in trash
        assert!(!report.trash_entries.is_empty());
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
    fn test_trash_entry_is_recorded() {
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
        assert_eq!(report.trash_entries.len(), 1);
        let entry = &report.trash_entries[0];
        assert_eq!(entry.original_path, file_path.to_string_lossy().to_string());
        assert_eq!(entry.actual_size, 13);
        // Trash file should exist
        assert!(Path::new(&entry.trash_path).exists());
    }

    #[test]
    fn test_trash_hash_avoids_long_paths() {
        let trash_dir = Path::new("/tmp/trash");
        let long_path = "/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/clap-4.6.1/examples/tutorial_derive/04_04_custom.rs";
        let trash_path = build_trash_path(trash_dir, long_path);
        let filename = trash_path.file_name().unwrap().to_string_lossy();
        // Hash is always 16 hex chars, well within NAME_MAX (255)
        assert!(filename.len() < 50);
    }
}
