// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal-precise display engine v3.
//!
//! Every row, border, and separator is exact-width-calculated.
//! Column widths computed from content, constrained to terminal width.
//! No overflow. No misalignment. No guesswork.

use crate::domain::DomainSummary;
use crate::rules::{ClassifiedFile, Decision};
use crate::summary::DecisionSummary;

const H: &str = "─";
const V: &str = "│";
const TL: &str = "┌";
const TR: &str = "┐";
const BL: &str = "└";
const BR: &str = "┘";
const LT: &str = "├";
const RT: &str = "┤";
const CR: &str = "┼";
const W: usize = 78;

/// Display width in characters (not bytes). Unicode box-drawing chars are 3 bytes but 1 column.
fn char_len(s: &str) -> usize {
    s.chars().count()
}

// ── Primitives ──

fn top() -> String {
    format!("{TL}{}{TR}", H.repeat(W - 2))
}
fn sep() -> String {
    format!("{LT}{}{RT}", H.repeat(W - 2))
}
fn bot() -> String {
    format!("{BL}{}{BR}", H.repeat(W - 2))
}

/// Build a mid-row separator matching column widths.
/// Layout: ├──W0──┼──W1──┼──...──┤
/// The `├` aligns with `│` in data rows.
fn mid_sep(widths: &[usize]) -> String {
    let mut s = LT.to_string();
    for (i, w) in widths.iter().enumerate() {
        s.push_str(&H.repeat(*w));
        if i < widths.len() - 1 {
            s.push_str(CR);
        }
    }
    let fill = W.saturating_sub(char_len(&s) + 1);
    s.push_str(&H.repeat(fill));
    s.push_str(RT);
    s
}

/// Render a header row: │COL1  COL2  COL3│
fn header_row(cols: &[&str], widths: &[usize]) -> String {
    row(cols, widths)
}

/// Render a data row: │val1  val2  val3│
fn data_row(vals: &[String], widths: &[usize]) -> String {
    let refs: Vec<&str> = vals.iter().map(|s| s.as_str()).collect();
    row(&refs, widths)
}

/// Core row renderer — exact width, no overflow.
/// Layout: │ cell0  cell1  cell2  │
fn row(cells: &[&str], widths: &[usize]) -> String {
    let mut s = format!("{V} ");
    for (i, cell) in cells.iter().enumerate() {
        let trimmed = truncate_cell(cell, widths[i]);
        s.push_str(&format!("{:w$}", trimmed, w = widths[i]));
        if i < widths.len() - 1 {
            s.push(' ');
        }
    }
    let fill = W.saturating_sub(char_len(&s) + 1);
    s.push_str(&" ".repeat(fill));
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

/// Compute column widths that fit within terminal width.
/// Accounts for: left border+space (2), right border (1), inter-column spaces (N-1).
fn fit_widths(headers: &[&str], n_cols: usize, min_w: usize) -> Vec<usize> {
    let separators = n_cols.saturating_sub(1);
    let borders = 3; // "│ " (2) + "│" (1)
    let available = W.saturating_sub(borders + separators);

    // Start with header widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len().max(min_w)).collect();
    let total: usize = widths.iter().sum();

    if total <= available {
        // Distribute remaining space proportionally
        let slack = available - total;
        for w in &mut widths {
            let add = slack * *w / total.max(1);
            *w += add;
        }
        // Distribute any remainder (from integer division) one-by-one
        let new_total: usize = widths.iter().sum();
        let mut leftover = available.saturating_sub(new_total);
        for w in widths.iter_mut().rev() {
            if leftover == 0 {
                break;
            }
            *w += 1;
            leftover -= 1;
        }
    } else {
        // Shrink proportionally
        let to_trim = total - available;
        for w in &mut widths {
            let cut = (to_trim * *w) / total;
            *w = (*w).saturating_sub(cut).max(min_w);
        }
    }
    widths
}

fn center_row(text: &str) -> String {
    let inner = W - 4; // │ _text_ │
    format!("{V} {:^inner$} {V}\n", text)
}

// ── Decision Summary ──

pub fn render_decision_summary(s: &DecisionSummary) -> String {
    let mut out = top();
    out.push('\n');
    out.push_str(&center_row("DECISION SUMMARY"));
    out.push_str(&sep());
    out.push('\n');

    let kv = |label: &str, value: &str| format!("{V} {:<40}{:>34} {V}\n", label, value);

    out.push_str(&kv("Files Found", &s.files_found.to_string()));
    out.push_str(&kv("Safe To Clean", &s.safe_to_clean.to_string()));
    out.push_str(&kv("Blocked", &s.blocked.to_string()));
    out.push_str(&kv("Recoverable Space", &human_size(s.recoverable_bytes)));
    out.push_str(&kv("Risk Level", &s.risk_level));

    out.push_str(&bot());
    out.push('\n');
    out
}

// ── Domain Summary ──

pub fn render_domain_summary(domains: &[DomainSummary]) -> String {
    if domains.is_empty() {
        return "No cache domains found.\n".to_string();
    }

    let headers = ["DOMAIN", "FILES", "SIZE", "RISK", "STATUS"];
    let widths = fit_widths(&headers, 5, 6);

    let mut out = top();
    out.push('\n');
    out.push_str(&center_row("CACHE DOMAIN SUMMARY"));
    out.push_str(&sep());
    out.push('\n');
    out.push_str(&header_row(&headers, &widths));
    out.push_str(&mid_sep(&widths));
    out.push('\n');

    for d in domains.iter().take(15) {
        let vals = [
            truncate_cell(&d.domain, widths[0]),
            d.file_count.to_string(),
            human_size(d.total_size),
            d.risk_label.clone(),
            d.status.label().to_string(),
        ]
        .map(|s| s.to_string());
        out.push_str(&data_row(&vals, &widths));
    }

    if domains.len() > 15 {
        out.push_str(&format!(
            "{V} {:>74} {V}\n",
            format!("... and {} more domains", domains.len() - 15)
        ));
    }

    out.push_str(&bot());
    out.push('\n');
    out
}

// ── File Table ──

pub fn render_table(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }

    let headers = ["FILE", "SIZE", "RISK", "STATUS"];
    let widths = [38, 8, 10, 15]; // pre-fit for 4 cols on 78-char terminal

    let mut out = top();
    out.push('\n');
    out.push_str(&center_row(&format!("ZACXIOM — {title}")));
    out.push_str(&sep());
    out.push('\n');
    out.push_str(&header_row(&headers, &widths));
    out.push_str(&mid_sep(&widths));
    out.push('\n');

    let body = render_collapsed(files, &widths, 35);
    out.push_str(&body);
    out.push_str(&bot());
    out.push('\n');
    out
}

fn render_collapsed(files: &[ClassifiedFile], widths: &[usize], max_rows: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut skipped = 0;

    while i < files.len() && i.saturating_sub(skipped) < max_rows {
        let f = &files[i];
        let mut dupes = 1;
        while i + dupes < files.len()
            && files[i + dupes].cache_domain == f.cache_domain
            && files[i + dupes].decision == f.decision
            && (files[i + dupes].risk_score - f.risk_score).abs() < 0.02
            && dupes < 200
        {
            dupes += 1;
        }

        if dupes >= 5 {
            let vals = [
                truncate_cell(&f.path, widths[0]),
                human_size(f.size),
                format!("{:.0}%", f.risk_score * 100.0),
                status_label(&f.decision).to_string(),
            ];
            out.push_str(&data_row(&vals, widths));
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
                status_label(&f.decision).to_string(),
            ];
            out.push_str(&data_row(&vals, widths));
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

    let headers = ["FILE", "SIZE", "RISK", "ACTION"];
    let widths = [34, 8, 10, 17]; // pre-fit for 4 cols

    let mut out = top();
    out.push('\n');
    out.push_str(&center_row(&format!("ZACXIOM SIMULATION — {title}")));
    out.push_str(&sep());
    out.push('\n');
    out.push_str(&header_row(&headers, &widths));
    out.push_str(&mid_sep(&widths));
    out.push('\n');

    let mut i = 0;
    let mut skipped = 0;
    while i < files.len() && i.saturating_sub(skipped) < 30 {
        let f = &files[i];
        let mut dupes = 1;
        while i + dupes < files.len()
            && files[i + dupes].decision == f.decision
            && files[i + dupes].cache_domain == f.cache_domain
            && dupes < 200
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
            out.push_str(&data_row(&vals, &widths));
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
            out.push_str(&data_row(&vals, &widths));
            i += 1;
        }
    }

    if i < files.len() {
        out.push_str(&format!(
            "{V} {:>74} {V}\n",
            format!("... and {} more files", files.len() - i)
        ));
    }

    out.push_str(&bot());
    out.push('\n');
    out
}

// ── Insight Footer ──

pub fn render_insights(ctx: &InsightContext) -> String {
    let mut out = top();
    out.push('\n');
    out.push_str(&center_row("INSIGHT"));
    out.push_str(&sep());
    out.push('\n');

    let safe_pct = if ctx.total > 0 {
        (ctx.safe as f64 / ctx.total as f64) * 100.0
    } else {
        0.0
    };
    let reclaimable = if ctx.safe + ctx.low_risk > 0 {
        (ctx.total_size as f64 * (ctx.safe + ctx.low_risk) as f64 / ctx.total.max(1) as f64) as u64
    } else {
        0
    };

    let risk_level = if ctx.high_risk > 0 {
        "HIGH — review before cleaning"
    } else if ctx.moderate > 5 {
        "MODERATE — use --smart"
    } else if ctx.safe > 0 {
        "LOW — safe to clean"
    } else {
        "MINIMAL — nothing actionable"
    };

    let mut rows = vec![format!(
        "{:.0}% of cache is safe to clean ({})",
        safe_pct,
        human_size(reclaimable)
    )];
    if ctx.open_files > 0 {
        rows.push(format!(
            "{} files held open by running processes",
            ctx.open_files
        ));
    }
    rows.push(format!("Risk level: {risk_level}"));
    if ctx.protected > 0 {
        rows.push(format!("{} system-protected files excluded", ctx.protected));
    }

    for r in &rows {
        out.push_str(&format!("{V}   {:<73} {V}\n", r));
    }

    out.push_str(&bot());
    out.push('\n');
    out
}

// ── Helpers ──

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
        assert!(top().contains("┌") && top().contains("┐"));
        assert!(sep().contains("├") && sep().contains("┤"));
        assert!(bot().contains("└") && bot().contains("┘"));
    }

    #[test]
    fn test_row_exact_width() {
        let widths = vec![10, 10, 10];
        let row_str = row(&["abc", "def", "ghi"], &widths);
        // Display width (chars) must equal W + newline
        assert_eq!(row_str.chars().count(), W + 1);
        assert!(row_str.starts_with('│'));
        assert!(row_str.trim_end().ends_with('│'));
    }

    #[test]
    fn test_mid_sep_aligns() {
        let widths = vec![10, 10, 10];
        let sep_line = mid_sep(&widths);
        let data = row(&["a", "b", "c"], &widths);
        // Both should span W display columns (data has trailing \n)
        assert_eq!(sep_line.chars().count(), W);
        assert_eq!(data.chars().count(), W + 1);
    }

    #[test]
    fn test_fit_widths_does_not_exceed() {
        let headers = ["DOMAIN", "FILES", "SIZE", "RISK", "STATUS"];
        let widths = fit_widths(&headers, 5, 5);
        let total: usize = widths.iter().sum();
        let separators = 4;
        let borders = 2;
        assert!(total + separators + borders <= W);
    }

    #[test]
    fn test_table_renders() {
        let files = vec![cf("/tmp/a", 1024, Decision::Safe)];
        let out = render_table(&files, "Test");
        assert!(out.contains("ELIGIBLE"));
        assert!(!out.contains("├──├"));
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
    }

    #[test]
    fn test_duplicate_collapse() {
        let mut files = Vec::new();
        for i in 0..10 {
            files.push(cf(&format!("/tmp/mesa_cache_{i}"), 100, Decision::Safe));
        }
        let out = render_table(&files, "Test");
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
    }

    #[test]
    fn test_truncate_cell() {
        assert_eq!(truncate_cell("hello", 10), "hello");
        assert_eq!(truncate_cell("hello world this is long", 10), "hello wo..");
    }
}
