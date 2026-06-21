// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal-precise display engine v4 — dynamic terminal width.
//!
//! Detects terminal width via ioctl(TIOCGWINSZ) on Linux.
//! Falls back to $COLUMNS env var, then 80 columns.
//! Minimum: 60 columns. All tables scale to available width.

use crate::domain::DomainSummary;
use crate::rules::{ClassifiedFile, Decision};
use crate::summary::DecisionSummary;
use std::sync::OnceLock;

const H: &str = "─";
const V: &str = "│";
const TL: &str = "┌";
const TR: &str = "┐";
const BL: &str = "└";
const BR: &str = "┘";
const LT: &str = "├";
const RT: &str = "┤";
const CR: &str = "┼";
const MIN_W: usize = 60;
const DEFAULT_W: usize = 80;

fn term_width() -> usize {
    static W: OnceLock<usize> = OnceLock::new();
    *W.get_or_init(|| detect_width().max(MIN_W))
}

fn detect_width() -> usize {
    #[cfg(target_os = "linux")]
    {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) } == 0 && ws.ws_col > 0 {
            return ws.ws_col as usize;
        }
    }
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(n) = cols.parse::<usize>() {
            if n > 0 {
                return n;
            }
        }
    }
    DEFAULT_W
}

/// All primitives now take explicit `w` parameter for width.
fn top(w: usize) -> String {
    format!("{TL}{}{TR}", H.repeat(w.saturating_sub(2)))
}
fn sep(w: usize) -> String {
    format!("{LT}{}{RT}", H.repeat(w.saturating_sub(2)))
}
fn bot(w: usize) -> String {
    format!("{BL}{}{BR}", H.repeat(w.saturating_sub(2)))
}

fn mid_sep(widths: &[usize], w: usize) -> String {
    let mut s = LT.to_string();
    for (i, cw) in widths.iter().enumerate() {
        s.push_str(&H.repeat(*cw));
        if i < widths.len() - 1 {
            s.push_str(CR);
        }
    }
    let fill = w.saturating_sub(char_len(&s) + 1);
    s.push_str(&H.repeat(fill));
    s.push_str(RT);
    s
}

fn row(cells: &[&str], widths: &[usize], w: usize) -> String {
    let mut s = format!("{V} ");
    for (i, cell) in cells.iter().enumerate() {
        let trimmed = truncate_cell(cell, widths[i]);
        s.push_str(&format!("{:cw$}", trimmed, cw = widths[i]));
        if i < widths.len() - 1 {
            s.push(' ');
        }
    }
    let fill = w.saturating_sub(char_len(&s) + 1);
    s.push_str(&" ".repeat(fill));
    s.push_str(V);
    s.push('\n');
    s
}

fn center_row(text: &str, w: usize) -> String {
    let inner = w.saturating_sub(4);
    format!("{V} {:^inner$} {V}\n", text)
}

/// Display width in characters (not bytes).
fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn truncate_cell(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        s[..max].to_string()
    } else {
        format!("{}..", &s[..max.saturating_sub(2)])
    }
}

fn fit_widths(headers: &[&str], n_cols: usize, min_w: usize, w: usize) -> Vec<usize> {
    let separators = n_cols.saturating_sub(1);
    let borders = 3;
    let available = w.saturating_sub(borders + separators);
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len().max(min_w)).collect();
    let total: usize = widths.iter().sum();
    if total <= available {
        let slack = available.saturating_sub(total);
        for cw in &mut widths {
            let add = slack * *cw / total.max(1);
            *cw += add;
        }
        let new_total: usize = widths.iter().sum();
        let mut leftover = available.saturating_sub(new_total);
        for cw in widths.iter_mut().rev() {
            if leftover == 0 {
                break;
            }
            *cw += 1;
            leftover -= 1;
        }
    } else {
        let to_trim = total.saturating_sub(available);
        for cw in &mut widths {
            let cut = (to_trim * *cw) / total;
            *cw = (*cw).saturating_sub(cut).max(min_w);
        }
    }
    widths
}

// ── Decision Summary ──

pub fn render_decision_summary(s: &DecisionSummary) -> String {
    let w = term_width();
    let lw = w * 55 / 100; // label width ~55%
    let vw = w - lw - 4; // value width = remaining minus borders/spaces

    let mut out = top(w);
    out.push('\n');
    out.push_str(&center_row("DECISION SUMMARY", w));
    out.push_str(&sep(w));
    out.push('\n');

    let kv = |label: &str, value: &str| {
        format!("{V} {:<lw$}{:>vw$} {V}\n", label, value, lw = lw, vw = vw)
    };

    out.push_str(&kv("Files Found", &s.files_found.to_string()));
    out.push_str(&kv("Safe To Clean", &s.safe_to_clean.to_string()));
    out.push_str(&kv("Blocked", &s.blocked.to_string()));
    out.push_str(&kv("Recoverable Space", &human_size(s.recoverable_bytes)));
    out.push_str(&kv("Risk Level", &s.risk_level));

    out.push_str(&bot(w));
    out.push('\n');
    out
}

// ── Domain Summary ──

pub fn render_domain_summary(domains: &[DomainSummary]) -> String {
    if domains.is_empty() {
        return "No cache domains found.\n".to_string();
    }
    let w = term_width();
    let headers = ["DOMAIN", "FILES", "SIZE", "RISK", "STATUS"];
    let widths = fit_widths(&headers, 5, 6, w);

    let mut out = top(w);
    out.push('\n');
    out.push_str(&center_row("CACHE DOMAIN SUMMARY", w));
    out.push_str(&sep(w));
    out.push('\n');
    out.push_str(&row(&headers, &widths, w));
    out.push_str(&mid_sep(&widths, w));
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
        let strs: Vec<&str> = vals.iter().map(|s| s.as_str()).collect();
        out.push_str(&row(&strs, &widths, w));
    }

    if domains.len() > 15 {
        out.push_str(&format!(
            "{V} {:>fill$} {V}\n",
            format!("... and {} more domains", domains.len() - 15),
            fill = w.saturating_sub(4)
        ));
    }

    out.push_str(&bot(w));
    out.push('\n');
    out
}

// ── File Table ──

pub fn render_table(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }
    let w = term_width();
    let headers = ["FILE", "SIZE", "RISK", "STATUS"];
    let widths = fit_widths(&headers, 4, 6, w);

    let mut out = top(w);
    out.push('\n');
    out.push_str(&center_row(&format!("ZACXIOM — {title}"), w));
    out.push_str(&sep(w));
    out.push('\n');
    out.push_str(&row(&headers, &widths, w));
    out.push_str(&mid_sep(&widths, w));
    out.push('\n');

    let body = render_collapsed(files, &widths, w);
    out.push_str(&body);
    out.push_str(&bot(w));
    out.push('\n');
    out
}

fn render_collapsed(files: &[ClassifiedFile], widths: &[usize], w: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut skipped = 0;
    let max_rows = w / 2; // scale rows to terminal height

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
            let strs: Vec<&str> = vals.iter().map(|s| s.as_str()).collect();
            out.push_str(&row(&strs, widths, w));
            out.push_str(&format!(
                "{V} {:<fill$} {V}\n",
                format!("  ... {} similar entries omitted", dupes - 1),
                fill = w.saturating_sub(4)
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
            let strs: Vec<&str> = vals.iter().map(|s| s.as_str()).collect();
            out.push_str(&row(&strs, widths, w));
            i += 1;
        }
    }

    if i < files.len() {
        out.push_str(&format!(
            "{V} {:>fill$} {V}\n",
            format!("... and {} more files", files.len() - i),
            fill = w.saturating_sub(4)
        ));
    }
    out
}

// ── Simulation Table ──

pub fn render_simulation(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }
    let w = term_width();
    let headers = ["FILE", "SIZE", "RISK", "ACTION"];
    let widths = fit_widths(&headers, 4, 6, w);

    let mut out = top(w);
    out.push('\n');
    out.push_str(&center_row(&format!("ZACXIOM SIMULATION — {title}"), w));
    out.push_str(&sep(w));
    out.push('\n');
    out.push_str(&row(&headers, &widths, w));
    out.push_str(&mid_sep(&widths, w));
    out.push('\n');

    let mut i = 0;
    let mut skipped = 0;
    let max_rows = w / 2;
    while i < files.len() && i.saturating_sub(skipped) < max_rows {
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
            let strs: Vec<&str> = vals.iter().map(|s| s.as_str()).collect();
            out.push_str(&row(&strs, &widths, w));
            out.push_str(&format!(
                "{V} {:<fill$} {V}\n",
                format!("  ... {} similar entries omitted", dupes - 1),
                fill = w.saturating_sub(4)
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
            let strs: Vec<&str> = vals.iter().map(|s| s.as_str()).collect();
            out.push_str(&row(&strs, &widths, w));
            i += 1;
        }
    }

    if i < files.len() {
        out.push_str(&format!(
            "{V} {:>fill$} {V}\n",
            format!("... and {} more files", files.len() - i),
            fill = w.saturating_sub(4)
        ));
    }

    out.push_str(&bot(w));
    out.push('\n');
    out
}

// ── Insight Footer ──

pub fn render_insights(ctx: &InsightContext) -> String {
    let w = term_width();
    let mut out = top(w);
    out.push('\n');
    out.push_str(&center_row("INSIGHT", w));
    out.push_str(&sep(w));
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
        "HIGH — review"
    } else if ctx.moderate > 5 {
        "MODERATE — use --smart"
    } else if ctx.safe > 0 {
        "LOW — safe"
    } else {
        "MINIMAL"
    };

    let rows: Vec<String> = vec![
        format!(
            "{:.0}% of cache is safe to clean ({})",
            safe_pct,
            human_size(reclaimable)
        ),
        format!("{} files open by running processes", ctx.open_files),
        format!("Risk: {risk_level}"),
    ];

    let fill = w.saturating_sub(5);
    for r in &rows {
        out.push_str(&format!("{V}  {:<fill$} {V}\n", r, fill = fill));
    }
    if ctx.protected > 0 {
        out.push_str(&format!(
            "{V}  {:<fill$} {V}\n",
            format!("{} system-protected files excluded", ctx.protected),
            fill = fill
        ));
    }

    out.push_str(&bot(w));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_w() -> usize {
        78
    }

    #[test]
    fn test_borders() {
        let w = test_w();
        assert!(top(w).contains("┌") && top(w).contains("┐"));
        assert!(sep(w).contains("├") && sep(w).contains("┤"));
        assert!(bot(w).contains("└") && bot(w).contains("┘"));
    }

    #[test]
    fn test_row_exact_width() {
        let w = test_w();
        let widths = vec![10, 10, 10];
        let row_str = row(&["abc", "def", "ghi"], &widths, w);
        assert_eq!(row_str.chars().count(), w + 1);
        assert!(row_str.starts_with('│'));
    }

    #[test]
    fn test_mid_sep_aligns() {
        let w = test_w();
        let widths = vec![10, 10, 10];
        let sep_line = mid_sep(&widths, w);
        let data = row(&["a", "b", "c"], &widths, w);
        assert_eq!(sep_line.chars().count(), w);
        assert_eq!(data.chars().count(), w + 1);
    }

    #[test]
    fn test_fit_widths_does_not_exceed() {
        let w = test_w();
        let headers = ["DOMAIN", "FILES", "SIZE", "RISK", "STATUS"];
        let widths = fit_widths(&headers, 5, 5, w);
        let total: usize = widths.iter().sum();
        let separators = 4;
        let borders = 3;
        assert!(total + separators + borders <= w);
    }

    #[test]
    fn test_dynamic_width_narrow() {
        let w = 60;
        let headers = ["A", "B", "C"];
        let widths = fit_widths(&headers, 3, 4, w);
        let total: usize = widths.iter().sum();
        assert!(total + 2 + 3 <= w); // 2 separators, 3 borders
    }
}
