// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Unknown Domain Intelligence — v6.3.2 observability layer.
//!
//! Answers: what's in the Unknown bucket? Where should rules be added?
//! Groups unknown files by common path prefixes for actionable insight.

use crate::rules::ClassifiedFile;
use crate::simulator;
use std::collections::HashMap;

/// A group of unknown files sharing a common path prefix.
pub struct UnknownGroup {
    pub prefix: String,
    pub file_count: usize,
    pub total_size: u64,
    pub pct_of_unknown: f32,
    /// Example paths in this group.
    pub examples: Vec<String>,
}

/// Breakdown of the Unknown classification bucket.
pub struct UnknownBreakdown {
    pub total_files: usize,
    pub unknown_files: usize,
    pub unknown_size: u64,
    pub coverage_pct: f32,
    pub groups: Vec<UnknownGroup>,
}

/// Analyze classified files and produce an Unknown breakdown.
pub fn analyze(files: &[ClassifiedFile]) -> UnknownBreakdown {
    let total = files.len();
    let unknown: Vec<&ClassifiedFile> = files
        .iter()
        .filter(|f| f.engine_category.is_empty() || f.engine_category == "Unknown")
        .collect();

    let unknown_count = unknown.len();
    let unknown_size: u64 = unknown.iter().map(|f| f.size).sum();
    let coverage = if total > 0 {
        (total - unknown_count) as f32 / total as f32 * 100.0
    } else {
        100.0
    };

    // Group by common path prefix (up to 4 components)
    let mut groups: HashMap<String, (usize, u64, Vec<String>)> = HashMap::new();

    for f in &unknown {
        let prefix = common_prefix(&f.path, 4);
        let entry = groups.entry(prefix).or_insert((0, 0, Vec::new()));
        entry.0 += 1;
        entry.1 += f.size;
        if entry.2.len() < 3 {
            entry.2.push(f.path.clone());
        }
    }

    let mut group_list: Vec<UnknownGroup> = groups
        .into_iter()
        .map(|(prefix, (count, size, examples))| UnknownGroup {
            prefix,
            file_count: count,
            total_size: size,
            pct_of_unknown: if unknown_count > 0 {
                count as f32 / unknown_count as f32 * 100.0
            } else {
                0.0
            },
            examples,
        })
        .collect();

    group_list.sort_by_key(|g| std::cmp::Reverse(g.total_size));

    UnknownBreakdown {
        total_files: total,
        unknown_files: unknown_count,
        unknown_size,
        coverage_pct: coverage,
        groups: group_list,
    }
}

/// Extract a common path prefix of up to `depth` components.
fn common_prefix(path: &str, depth: usize) -> String {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let n = parts.len().min(depth);
    if n == 0 {
        return "/".into();
    }
    let prefix = parts[..n].join("/");
    if path.starts_with('/') {
        format!("/{prefix}")
    } else {
        prefix
    }
}

/// Render the unknown breakdown report.
pub fn render_report(breakdown: &UnknownBreakdown) -> String {
    let mut out = String::new();

    out.push_str(&format!("\nUNKNOWN BREAKDOWN\n{}\n\n", "─".repeat(60)));
    out.push_str(&format!("  Total Files:    {}\n", breakdown.total_files));
    out.push_str(&format!(
        "  Unknown:        {} ({})\n",
        breakdown.unknown_files,
        simulator::human_size(breakdown.unknown_size)
    ));
    out.push_str(&format!(
        "  Coverage:       {:.1}%\n",
        breakdown.coverage_pct
    ));

    if breakdown.groups.is_empty() {
        out.push_str("\n  No unknown files found.\n");
        return out;
    }

    out.push_str(&format!(
        "\n  TOP UNKNOWN PREFIXES ({} groups)\n\n",
        breakdown.groups.len().min(25)
    ));

    for (i, g) in breakdown.groups.iter().take(25).enumerate() {
        out.push_str(&format!(
            "  {:>2}. {:<45} {:>5} files  {}\n",
            i + 1,
            truncate(&g.prefix, 45),
            g.file_count,
            simulator::human_size(g.total_size),
        ));
        if g.pct_of_unknown > 5.0 {
            for ex in &g.examples {
                out.push_str(&format!("       ex: {}\n", truncate(ex, 55)));
            }
        }
    }

    if breakdown.groups.len() > 25 {
        out.push_str(&format!(
            "\n  ... and {} more prefix groups\n",
            breakdown.groups.len() - 25
        ));
    }

    out
}

/// Render coverage summary line (for scan output footer).
pub fn render_coverage(breakdown: &UnknownBreakdown) -> String {
    format!(
        "\nCLASSIFIER COVERAGE\n{}\n  Known:     {} files\n  Unknown:   {} files\n  Coverage:  {:.1}%\n",
        "─".repeat(40),
        breakdown.total_files - breakdown.unknown_files,
        breakdown.unknown_files,
        breakdown.coverage_pct
    )
}

/// Export breakdown as JSON.
pub fn render_json(breakdown: &UnknownBreakdown) -> String {
    let groups: Vec<serde_json::Value> = breakdown
        .groups
        .iter()
        .take(50)
        .map(|g| {
            serde_json::json!({
                "prefix": g.prefix,
                "files": g.file_count,
                "size": g.total_size,
                "size_human": simulator::human_size(g.total_size),
                "pct_of_unknown": format!("{:.1}%", g.pct_of_unknown),
                "examples": g.examples,
            })
        })
        .collect();

    let out = serde_json::json!({
        "total_files": breakdown.total_files,
        "unknown_files": breakdown.unknown_files,
        "unknown_size": breakdown.unknown_size,
        "unknown_size_human": simulator::human_size(breakdown.unknown_size),
        "coverage_pct": format!("{:.1}", breakdown.coverage_pct),
        "top_unknown": groups,
    });

    serde_json::to_string_pretty(&out).unwrap_or_default()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        s[..max].to_string()
    } else {
        format!("{}..", &s[..max.saturating_sub(2)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, ClassifiedFile, Decision, Ownership};

    fn make_file(path: &str, size: u64, engine_cat: &str) -> ClassifiedFile {
        ClassifiedFile {
            path: path.to_string(),
            size,
            cache_domain: CacheDomain::Unknown,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.0,
            risk_reasons: vec![],
            decision: Decision::Moderate,
            engine_category: engine_cat.to_string(),
            engine_confidence: 0,
        }
    }

    #[test]
    fn test_coverage_calculation() {
        let files = vec![
            make_file("/home/user/.cache/brave/a", 100, "Browser Cache"),
            make_file("/home/user/.cache/brave/b", 200, "Browser Cache"),
            make_file("/home/user/mystery/file", 300, ""),
            make_file("/home/user/mystery/file2", 400, "Unknown"),
        ];
        let b = analyze(&files);
        assert_eq!(b.total_files, 4);
        assert_eq!(b.unknown_files, 2);
        assert_eq!(b.unknown_size, 700);
        assert!((b.coverage_pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_groups_form() {
        let files = vec![
            make_file("/home/user/.local/share/icons/a", 100, ""),
            make_file("/home/user/.local/share/icons/b", 200, ""),
            make_file("/home/user/.config/gtk-3.0/a", 300, ""),
        ];
        let b = analyze(&files);
        assert!(b.groups.len() >= 2);
    }

    #[test]
    fn test_all_known_is_100() {
        let files = vec![
            make_file("/home/user/.cache/a", 100, "Browser Cache"),
            make_file("/home/user/.cargo/a", 200, "Downloaded Artifact"),
        ];
        let b = analyze(&files);
        assert!((b.coverage_pct - 100.0).abs() < 0.1);
    }
}
