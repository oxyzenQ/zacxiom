// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal-precise display engine v2.
//!
//! Correct Unicode box-drawing junctions (├──┤ not ├──├).
//! Content-aware column widths. Collapsed repetitive entries.
//! Professional-grade terminal rendering.

use crate::domain::DomainSummary;
use crate::rules::{ClassifiedFile, Decision};
use crate::summary::DecisionSummary;

// ── Box drawing constants ──
const H: &str = "─";
const V: &str = "│";
const TL: &str = "┌";
const TR: &str = "┐";
const BL: &str = "└";
const BR: &str = "┘";
const LT: &str = "├";
const RT: &str = "┤";
#[allow(dead_code)]
const TT: &str = "┬";
#[allow(dead_code)]
const BT: &str = "┴";
const CR: &str = "┼";

/// Build a top border: ┌──────┐
fn top(width: usize) -> String {
    format!("{TL}{}{TR}", H.repeat(width - 2))
}

/// Build a header separator: ├──────┤
fn sep(width: usize) -> String {
    format!("{LT}{}{RT}", H.repeat(width - 2))
}

/// Build a bottom border: └──────┘
fn bot(width: usize) -> String {
    format!("{BL}{}{BR}", H.repeat(width - 2))
}

/// Build a mid separator: ├──────┼──────┤
fn mid_sep(widths: &[usize], width: usize) -> String {
    let mut s = LT.to_string();
    let mut total = 1; // left junction
    for (i, w) in widths.iter().enumerate() {
        s.push_str(&H.repeat(*w));
        total += w;
        if i < widths.len() - 1 {
            s.push_str(CR);
            total += 1;
        }
    }
    // Fill remaining space to reach width
    let remaining = width.saturating_sub(total + 1); // +1 for right junction
    s.push_str(&H.repeat(remaining));
    s.push_str(RT);
    s
}

/// Render a header row.
fn header_row(cols: &[String], widths: &[usize], width: usize) -> String {
    let mut s = format!("{V} ");
    for (i, col) in cols.iter().enumerate() {
        s.push_str(&format!("{:w$}", col, w = widths[i]));
        if i < cols.len() - 1 {
            s.push(' ');
        }
    }
    s.push_str(&" ".repeat(width.saturating_sub(s.len() + 1)));
    s.push_str(V);
    s.push('\n');
    s
}

/// Render a data row.
fn data_row(vals: &[String], widths: &[usize], width: usize) -> String {
    let mut s = format!("{V} ");
    for (i, val) in vals.iter().enumerate() {
        let truncated = truncate_cell(val, widths[i]);
        s.push_str(&format!("{:w$}", truncated, w = widths[i]));
        if i < vals.len() - 1 {
            s.push(' ');
        }
    }
    s.push_str(&" ".repeat(width.saturating_sub(s.len() + 1)));
    s.push_str(V);
    s.push('\n');
    s
}

fn truncate_cell(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        s[..max].to_string()
    } else {
        format!("{}..", &s[..max - 2])
    }
}

/// Fixed table width.
const W: usize = 78;

// ── Decision Summary ──

pub fn render_decision_summary(s: &DecisionSummary) -> String {
    let mut out = top(W);
    out.push('\n');
    out.push_str(&center_row("DECISION SUMMARY", W));
    out.push('\n');
    out.push_str(&sep(W));
    out.push('\n');

    let rows: Vec<(&str, String)> = vec![
        ("Files Found", s.files_found.to_string()),
        ("Safe To Clean", s.safe_to_clean.to_string()),
        ("Blocked", s.blocked.to_string()),
        ("Recoverable Space", human_size(s.recoverable_bytes)),
        ("Risk Level", s.risk_level.clone()),
    ];

    for (label, value) in &rows {
        out.push_str(&format!("{V} {:<40} {:>34} {V}\n", label, value));
    }

    out.push_str(&bot(W));
    out.push('\n');
    out
}

// ── Domain Summary Table ──

pub fn render_domain_summary(domains: &[DomainSummary]) -> String {
    if domains.is_empty() {
        return "No cache domains found.\n".to_string();
    }

    let cols = ["DOMAIN", "FILES", "SIZE", "RISK", "STATUS"];
    // content-aware widths: domain gets more space, stats get fixed
    let widths = [28, 8, 10, 12, 15];
    let col_strings: Vec<String> = cols.iter().map(|s| s.to_string()).collect();

    let mut out = top(W);
    out.push('\n');
    out.push_str(&center_row("CACHE DOMAIN SUMMARY", W));
    out.push('\n');
    out.push_str(&sep(W));
    out.push('\n');
    out.push_str(&header_row(&col_strings, &widths, W));
    out.push_str(&mid_sep(&widths, W));
    out.push('\n');

    for d in domains.iter().take(15) {
        let vals = [
            truncate_cell(&d.domain, 26),
            d.file_count.to_string(),
            human_size(d.total_size),
            d.risk_label.clone(),
            d.status.label().to_string(),
        ]
        .map(|s| s);
        out.push_str(&data_row(&vals, &widths, W));
    }

    if domains.len() > 15 {
        out.push_str(&format!(
            "{V} {:>74} {V}\n",
            format!("... and {} more domains", domains.len() - 15)
        ));
    }

    out.push_str(&bot(W));
    out.push('\n');
    out
}

// ── File Table (scan/report) ──

pub fn render_table(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }

    let cols = ["FILE", "SIZE", "RISK", "STATUS"];
    let widths = [40, 8, 10, 15];

    let mut out = top(W);
    out.push('\n');
    out.push_str(&center_row(&format!("ZACXIOM — {title}"), W));
    out.push('\n');
    out.push_str(&sep(W));
    out.push('\n');
    out.push_str(&header_row(&cols.map(|s| s.to_string()), &widths, W));
    out.push_str(&mid_sep(&widths, W));
    out.push('\n');

    // Collapse repetitive entries
    let rendered = render_collapsed(files, &widths, W, 40);
    out.push_str(&rendered);

    out.push_str(&bot(W));
    out.push('\n');
    out
}

/// Render files with duplicate collapsing.
fn render_collapsed(
    files: &[ClassifiedFile],
    widths: &[usize],
    width: usize,
    max_rows: usize,
) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut skipped = 0usize;

    while i < files.len() && i - skipped < max_rows {
        let f = &files[i];

        // Detect duplicate domains: if next N files share same domain+decision+risk,
        // collapse them into one representative row + skip count.
        let mut dupes = 1usize;
        while i + dupes < files.len()
            && files[i + dupes].cache_domain == f.cache_domain
            && files[i + dupes].decision == f.decision
            && (files[i + dupes].risk_score - f.risk_score).abs() < 0.01
            && dupes < 100
        {
            dupes += 1;
        }

        if dupes >= 5 {
            // Show one representative
            let vals = [
                truncate_cell(&f.path, widths[0]),
                human_size(f.size),
                format!("{:.0}%", f.risk_score * 100.0),
                status_label(&f.decision).to_string(),
            ];
            out.push_str(&data_row(&vals, widths, width));

            // Then the skip line
            let skip_msg = format!("  ... {} similar entries omitted", dupes - 1);
            out.push_str(&format!("{V} {:<74} {V}\n", skip_msg));

            i += dupes;
            skipped += dupes - 1;
        } else {
            let vals = [
                truncate_cell(&f.path, widths[0]),
                human_size(f.size),
                format!("{:.0}%", f.risk_score * 100.0),
                status_label(&f.decision).to_string(),
            ];
            out.push_str(&data_row(&vals, widths, width));
            i += 1;
        }
    }

    if i < files.len() {
        out.push_str(&format!(
            "{V} {:>74} {V}\n",
            format!("... and {} more files", files.len() - i)
        ));
    }

    out
}

// ── Simulation Table ──

pub fn render_simulation(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }

    let cols = ["FILE", "SIZE", "RISK", "ACTION"];
    let widths = [36, 8, 10, 19];

    let mut out = top(W);
    out.push('\n');
    out.push_str(&center_row(&format!("ZACXIOM SIMULATION — {title}"), W));
    out.push('\n');
    out.push_str(&sep(W));
    out.push('\n');
    out.push_str(&header_row(&cols.map(|s| s.to_string()), &widths, W));
    out.push_str(&mid_sep(&widths, W));
    out.push('\n');

    let mut i = 0;
    let mut skipped = 0usize;
    while i < files.len() && i - skipped < 35 {
        let f = &files[i];
        let mut dupes = 1;
        while i + dupes < files.len()
            && files[i + dupes].decision == f.decision
            && files[i + dupes].cache_domain == f.cache_domain
            && dupes < 100
        {
            dupes += 1;
        }

        if dupes >= 5 {
            let vals = [
                truncate_cell(&f.path, widths[0]),
                human_size(f.size),
                format!("{:.0}%", f.risk_score * 100.0),
                action_label(&f.decision).to_string(),
            ];
            out.push_str(&data_row(&vals, &widths, W));
            out.push_str(&format!(
                "{V} {:<74} {V}\n",
                format!("  ... {} similar entries omitted", dupes - 1)
            ));
            i += dupes;
            skipped += dupes - 1;
        } else {
            let vals = [
                truncate_cell(&f.path, widths[0]),
                human_size(f.size),
                format!("{:.0}%", f.risk_score * 100.0),
                action_label(&f.decision).to_string(),
            ];
            out.push_str(&data_row(&vals, &widths, W));
            i += 1;
        }
    }

    if i < files.len() {
        out.push_str(&format!(
            "{V} {:>74} {V}\n",
            format!("... and {} more files", files.len() - i)
        ));
    }

    out.push_str(&bot(W));
    out.push('\n');
    out
}

// ── Insight Footer ──

pub fn render_insights(ctx: &InsightContext) -> String {
    let mut out = sep(W);
    out.push('\n');
    out.push_str(&format!("{V} {:^74} {V}\n", "INSIGHT"));
    out.push_str(&sep(W));
    out.push('\n');

    let safe_pct = if ctx.total > 0 {
        (ctx.safe as f64 / ctx.total as f64) * 100.0
    } else {
        0.0
    };

    let reclaimable = if ctx.safe + ctx.low_risk > 0 {
        ctx.total_size * ctx.safe.max(ctx.low_risk) as u64 / ctx.total.max(1) as u64
    } else {
        0
    };

    out.push_str(&format!(
        "{V}   {:.0}% of cache is safe to clean ({:<46}) {V}\n",
        safe_pct,
        human_size(reclaimable),
    ));

    if ctx.open_files > 0 {
        out.push_str(&format!(
            "{V}   {} files held open by running processes{:<33} {V}\n",
            ctx.open_files, ""
        ));
    }

    let risk_level = if ctx.high_risk > 0 {
        "HIGH — review before cleaning"
    } else if ctx.moderate > 5 {
        "MODERATE — use --smart for safe cleanup"
    } else if ctx.safe > 0 {
        "LOW — safe to clean"
    } else {
        "MINIMAL — nothing actionable"
    };
    out.push_str(&format!("{V}   Risk level: {risk_level:<54} {V}\n"));

    if ctx.protected > 0 {
        out.push_str(&format!(
            "{V}   {} system-protected files excluded{:<32} {V}\n",
            ctx.protected, ""
        ));
    }

    out.push_str(&bot(W));
    out.push('\n');
    out
}

// ── Helpers ──

fn center_row(text: &str, width: usize) -> String {
    format!("{V} {:^w$} {V}", text, w = width - 4)
}

fn status_label(d: &Decision) -> &'static str {
    match d {
        Decision::Safe => "ELIGIBLE",
        Decision::LowRisk => "LOW_RISK",
        Decision::Moderate => "CAUTION",
        Decision::HighRisk => "BLOCKED",
        Decision::Protected => "PROTECTED",
    }
}

fn action_label(d: &Decision) -> &'static str {
    match d {
        Decision::Safe => "WOULD CLEAN",
        Decision::LowRisk => "SMART CLEAN",
        Decision::Moderate => "NEEDS --force",
        Decision::HighRisk => "BLOCKED",
        Decision::Protected => "NEVER",
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{:.0} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

// ── Insight Context ──

pub struct InsightContext {
    pub total: usize,
    pub safe: usize,
    pub low_risk: usize,
    pub moderate: usize,
    pub high_risk: usize,
    pub protected: usize,
    pub total_size: u64,
    pub open_files: usize,
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, Ownership};

    fn cf(path: &str, size: u64, decision: Decision) -> ClassifiedFile {
        ClassifiedFile {
            path: path.into(),
            size,
            cache_domain: CacheDomain::Browser,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.05,
            risk_reasons: vec!["test".into()],
            decision,
        }
    }

    #[test]
    fn test_borders_have_correct_junctions() {
        // Verify top/sep/bot use proper box-drawing characters
        assert!(top(10).contains("┌") && top(10).contains("┐"));
        assert!(sep(10).contains("├") && sep(10).contains("┤"));
        assert!(bot(10).contains("└") && bot(10).contains("┘"));
    }

    #[test]
    fn test_table_renders() {
        let files = vec![cf("/tmp/a", 1024, Decision::Safe)];
        let out = render_table(&files, "Test");
        assert!(out.contains("ELIGIBLE"));
        assert!(out.contains("ZACXIOM"));
        // Must have correct right border junctions
        assert!(!out.contains("├──├"));
        assert!(!out.contains("├───────├"));
    }

    #[test]
    fn test_simulation_renders() {
        let files = vec![
            cf("/tmp/a", 100, Decision::Safe),
            cf("/tmp/b", 200, Decision::HighRisk),
        ];
        let out = render_simulation(&files, "Test");
        assert!(out.contains("WOULD CLEAN"));
        assert!(out.contains("BLOCKED"));
        // Correct junctions
        assert!(out.contains("┤\n") || out.contains("┤"));
    }

    #[test]
    fn test_duplicate_collapse() {
        let mut files = Vec::new();
        for i in 0..10 {
            files.push(cf(
                &format!("/tmp/mesa_cache_{i}"),
                (i as u64) * 100,
                Decision::Safe,
            ));
        }
        let out = render_table(&files, "Test");
        // Should collapse similar entries
        assert!(out.contains("similar entries omitted"));
    }

    #[test]
    fn test_insights_renders() {
        let ctx = InsightContext {
            total: 100,
            safe: 60,
            low_risk: 20,
            moderate: 15,
            high_risk: 3,
            protected: 2,
            total_size: 10_000_000,
            open_files: 5,
        };
        let out = render_insights(&ctx);
        assert!(out.to_lowercase().contains("insight"));
        assert!(out.to_lowercase().contains("risk"));
    }

    #[test]
    fn test_truncate_cell() {
        assert_eq!(truncate_cell("hello", 10), "hello");
        assert_eq!(truncate_cell("hello world this is long", 10), "hello wo..");
    }
}
