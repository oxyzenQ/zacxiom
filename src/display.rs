// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal display engine v5 — clean, minimal, readable.
//!
//! v6.2.2: Removed all box-drawing characters. Single visual language.
//! Uses simple `───` section headers. Works on any terminal width.
//! Readable over SSH. Readable on 80x24.

use crate::confidence::ConfidenceSummary;
use crate::domain::DomainSummary;
use crate::rules::{ClassifiedFile, Decision};
use crate::summary::DecisionSummary;
use std::sync::OnceLock;

const MIN_W: usize = 40;
const DEFAULT_W: usize = 80;

fn term_width() -> usize {
    static W: OnceLock<usize> = OnceLock::new();
    *W.get_or_init(|| detect_width().max(MIN_W))
}

fn detect_width() -> usize {
    #[cfg(unix)]
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

/// Section header — purple accent, plain fallback.
pub fn section(name: &str) -> String {
    crate::color::section_header(name)
}

/// Key-value line: label:  value
pub fn kv(label: &str, value: &str) -> String {
    format!("{label:<30} {value}\n")
}

/// Render a single key-value pair tightly.
pub fn kv_line(label: &str, value: &str, _w: usize) -> String {
    let lw = (_w / 3).min(28);
    format!("  {label:<0$} {value}\n", lw)
}

/// Compact domain list — no boxes, just aligned columns.
pub fn render_domains_compact(domains: &[DomainSummary]) -> String {
    if domains.is_empty() {
        return String::new();
    }
    let mut out = section("DOMAIN SUMMARY");
    for d in domains.iter().take(20) {
        out.push_str(&format!(
            "  {:<25} {:>4} files  {:>8}  {}\n",
            truncate(&d.domain, 25),
            d.file_count,
            human_size(d.total_size),
            d.risk_label
        ));
    }
    if domains.len() > 20 {
        out.push_str(&format!("  ... and {} more domains\n", domains.len() - 20));
    }
    out
}

/// Decision summary — clean key-value style.
pub fn render_decision_summary(s: &DecisionSummary) -> String {
    let w = term_width();
    let mut out = section("DECISION SUMMARY");
    out.push_str(&kv_line("Files Found", &s.files_found.to_string(), w));
    out.push_str(&kv_line("Safe To Clean", &s.safe_to_clean.to_string(), w));
    out.push_str(&kv_line("Blocked", &s.blocked.to_string(), w));
    out.push_str(&kv_line("Recoverable", &human_size(s.recoverable_bytes), w));
    out.push_str(&kv_line("Risk Level", &s.risk_level, w));
    out
}

/// Domain summary — kept for backward compat, delegates to compact.
pub fn render_domain_summary(domains: &[DomainSummary]) -> String {
    render_domains_compact(domains)
}

/// Confidence summary — tier list, no boxes.
pub fn render_confidence_summary(cs: &ConfidenceSummary) -> String {
    let total = cs.total.max(1) as f64;
    let bar = |count: usize| -> String {
        if count == 0 {
            return "—".into();
        }
        let pct = count as f64 / total * 100.0;
        format!("{} ({:.0}%)", count, pct)
    };

    let mut out = section("CONFIDENCE BREAKDOWN");
    if cs.maximum > 0 {
        out.push_str(&format!("  ★★★★★ Safe        {}\n", bar(cs.maximum)));
    }
    if cs.high > 0 {
        out.push_str(&format!("  ★★★★  Low Risk     {}\n", bar(cs.high)));
    }
    if cs.moderate > 0 {
        out.push_str(&format!("  ★★★   Review       {}\n", bar(cs.moderate)));
    }
    if cs.low > 0 {
        out.push_str(&format!("  ★★    Caution      {}\n", bar(cs.low)));
    }
    if cs.minimal > 0 {
        out.push_str(&format!("  ★     Manual       {}\n", bar(cs.minimal)));
    }
    if cs.protected > 0 {
        out.push_str(&format!("  ⛔     Protected    {}\n", bar(cs.protected)));
    }
    out.push_str(&format!(
        "  Safe to auto-clean:  {} files\n",
        cs.cleanable_default()
    ));
    out.push_str(&format!(
        "  With --smart:        {} files\n",
        cs.cleanable_smart()
    ));
    out
}

/// File table — compact list, no boxes. Title is a section header.
pub fn render_table(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files: {title}\n");
    }
    let w = term_width();
    let mut out = section(title);
    for f in files
        .iter()
        .take(if files.len() > 50 { 40 } else { files.len() })
    {
        out.push_str(&format!(
            "  {:<40} {:>8}  {}\n",
            truncate(&f.path, (w / 2).min(40)),
            human_size(f.size),
            status_label(&f.decision)
        ));
    }
    if files.len() > 50 {
        out.push_str(&format!("  ... and {} more files\n", files.len() - 40));
    }
    out
}

/// Simulation table — compact list.
pub fn render_simulation(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files: {title}\n");
    }
    let w = term_width();
    let mut out = section(&format!("SIMULATION — {title}"));
    for f in files
        .iter()
        .take(if files.len() > 50 { 40 } else { files.len() })
    {
        out.push_str(&format!(
            "  {:<40} {:>8}  {}\n",
            truncate(&f.path, (w / 2).min(40)),
            human_size(f.size),
            action_label(&f.decision)
        ));
    }
    if files.len() > 50 {
        out.push_str(&format!("  ... and {} more files\n", files.len() - 40));
    }
    out
}

// ── Helpers ──

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        s[..max].to_string()
    } else {
        format!("{}..", &s[..max.saturating_sub(2)])
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

fn status_label(d: &Decision) -> &'static str {
    match d {
        Decision::Safe => "SAFE",
        Decision::LowRisk => "LOW_RISK",
        Decision::Moderate => "CAUTION",
        Decision::HighRisk => "BLOCKED",
        Decision::Protected => "PROTECTED",
        Decision::ProtectedActiveEnvironment => "PROTECTED",
    }
}

fn action_label(d: &Decision) -> &'static str {
    match d {
        Decision::Safe => "WOULD CLEAN",
        Decision::LowRisk => "SMART",
        Decision::Moderate => "FORCE",
        Decision::HighRisk => "BLOCKED",
        Decision::Protected => "NEVER",
        Decision::ProtectedActiveEnvironment => "NEVER",
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
}

pub fn render_insights(ctx: &InsightContext) -> String {
    let safe_pct = if ctx.total > 0 {
        (ctx.safe as f64 / ctx.total as f64) * 100.0
    } else {
        0.0
    };

    let risk_level = if ctx.high_risk > 0 {
        "HIGH — review carefully"
    } else if ctx.moderate > 5 {
        "MODERATE — use --smart"
    } else if ctx.safe > 0 {
        "LOW — safe to clean"
    } else {
        "MINIMAL"
    };

    let mut out = section("INSIGHT");
    out.push_str(&format!("  {:.0}% of cache is safe to clean\n", safe_pct));
    out.push_str(&format!("  Risk: {risk_level}\n"));
    if ctx.protected > 0 {
        out.push_str(&format!(
            "  {} system-protected files excluded\n",
            ctx.protected
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_format() {
        let s = section("TEST");
        assert!(s.contains("TEST"));
        assert!(s.contains("─"));
        assert!(!s.contains("┌"));
        assert!(!s.contains("╔"));
    }

    #[test]
    fn test_human_size() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1048576), "1.0 MB");
        assert_eq!(human_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_no_box_characters_in_output() {
        // All render functions must be free of box-drawing chars
        let cs = ConfidenceSummary::empty();
        let out = render_confidence_summary(&cs);
        assert!(!out.contains('┌'));
        assert!(!out.contains('┐'));
        assert!(!out.contains('╔'));
        assert!(!out.contains('╚'));
        assert!(!out.contains('├'));
        assert!(!out.contains('┤'));
    }
}
