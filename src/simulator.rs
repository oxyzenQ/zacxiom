// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dry-run simulator — mandatory before any clean operation.
//!
//! Produces explainable output showing exactly what WOULD happen
//! without touching a single file. Required by H5.

use crate::rules::{ClassifiedFile, Decision};
use serde::Serialize;

/// Result of a simulation run.
#[derive(Debug, Serialize)]
pub struct SimulationReport {
    pub total_files: usize,
    pub total_size: u64,
    pub safe_count: usize,
    pub safe_size: u64,
    pub low_risk_count: usize,
    pub low_risk_size: u64,
    pub moderate_count: usize,
    pub moderate_size: u64,
    pub high_risk_count: usize,
    pub high_risk_size: u64,
    pub protected_count: usize,
    pub protected_size: u64,
    pub entries: Vec<SimulationEntry>,
}

/// A single file's simulation entry in the standard output format.
#[derive(Debug, Serialize)]
pub struct SimulationEntry {
    pub path: String,
    pub size: u64,
    pub cache_domain: String,
    pub ownership: String,
    pub risk_score: f64,
    pub decision: String,
    pub reason: String,
}

/// Run simulation over classified files.
pub fn simulate(files: &[ClassifiedFile]) -> SimulationReport {
    let mut report = SimulationReport {
        total_files: files.len(),
        total_size: files.iter().map(|f| f.size).sum(),
        safe_count: 0,
        safe_size: 0,
        low_risk_count: 0,
        low_risk_size: 0,
        moderate_count: 0,
        moderate_size: 0,
        high_risk_count: 0,
        high_risk_size: 0,
        protected_count: 0,
        protected_size: 0,
        entries: Vec::with_capacity(files.len()),
    };

    for file in files {
        let (decision_str, count_field, size_field) = match &file.decision {
            Decision::Safe => ("SAFE", &mut report.safe_count, &mut report.safe_size),
            Decision::LowRisk => (
                "LOW_RISK",
                &mut report.low_risk_count,
                &mut report.low_risk_size,
            ),
            Decision::Moderate => (
                "MODERATE",
                &mut report.moderate_count,
                &mut report.moderate_size,
            ),
            Decision::HighRisk => (
                "HIGH_RISK",
                &mut report.high_risk_count,
                &mut report.high_risk_size,
            ),
            Decision::Protected => (
                "PROTECTED",
                &mut report.protected_count,
                &mut report.protected_size,
            ),
        };

        *count_field += 1;
        *size_field += file.size;

        report.entries.push(SimulationEntry {
            path: file.path.clone(),
            size: file.size,
            cache_domain: file.cache_domain.to_string(),
            ownership: file.ownership.to_string(),
            risk_score: file.risk_score,
            decision: decision_str.to_string(),
            reason: file.risk_reasons.join("; "),
        });
    }

    report
}

/// Format a simulation report as human-readable text (the standard format).
pub fn format_report(report: &SimulationReport) -> String {
    let mut out = String::new();

    out.push_str("═══════════════════════════════════════════\n");
    out.push_str("  ZACXIOM SIMULATION REPORT\n");
    out.push_str("  file → reason → risk → decision\n");
    out.push_str("═══════════════════════════════════════════\n\n");

    for entry in &report.entries {
        out.push_str(&format!(
            "  {} → {} → {:.2} → {} ({})\n",
            entry.path, entry.cache_domain, entry.risk_score, entry.decision, entry.reason
        ));
    }

    out.push_str("\n───────────────────────────────────────────\n");
    out.push_str("  SUMMARY\n");
    out.push_str("───────────────────────────────────────────\n");
    out.push_str(&format!("  Total files scanned : {}\n", report.total_files));
    out.push_str(&format!(
        "  Total size          : {} ({})\n",
        report.total_size,
        human_size(report.total_size)
    ));
    out.push_str(&format!(
        "  SAFE                : {} files, {}\n",
        report.safe_count,
        human_size(report.safe_size)
    ));
    out.push_str(&format!(
        "  LOW_RISK            : {} files, {}\n",
        report.low_risk_count,
        human_size(report.low_risk_size)
    ));
    out.push_str(&format!(
        "  MODERATE            : {} files, {}\n",
        report.moderate_count,
        human_size(report.moderate_size)
    ));
    out.push_str(&format!(
        "  HIGH_RISK           : {} files, {}\n",
        report.high_risk_count,
        human_size(report.high_risk_size)
    ));
    out.push_str(&format!(
        "  PROTECTED           : {} files, {}\n",
        report.protected_count,
        human_size(report.protected_size)
    ));
    out.push_str("═══════════════════════════════════════════\n");

    out
}

/// Human-readable byte sizes.
pub fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};

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
    fn test_simulate_counts_correctly() {
        let files = vec![
            make_file("/a", 100, Decision::Safe),
            make_file("/b", 200, Decision::Safe),
            make_file("/c", 50, Decision::LowRisk),
            make_file("/d", 500, Decision::Protected),
        ];

        let report = simulate(&files);
        assert_eq!(report.total_files, 4);
        assert_eq!(report.total_size, 850);
        assert_eq!(report.safe_count, 2);
        assert_eq!(report.safe_size, 300);
        assert_eq!(report.low_risk_count, 1);
        assert_eq!(report.low_risk_size, 50);
        assert_eq!(report.protected_count, 1);
        assert_eq!(report.protected_size, 500);
    }

    #[test]
    fn test_human_size() {
        assert_eq!(human_size(0), "0.00 B");
        assert_eq!(human_size(1024), "1.00 KB");
        assert_eq!(human_size(1048576), "1.00 MB");
        assert_eq!(human_size(1073741824), "1.00 GB");
    }
}
