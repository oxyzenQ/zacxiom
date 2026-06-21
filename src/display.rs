// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Table-first display engine — structured output, not log spam.
//!
//! Principles: clarity over verbosity, structure over logs, insight over output.

use crate::rules::{ClassifiedFile, Decision};

const SEP: &str = "─";
const WIDE: usize = 78;

/// Render a table of classified files.
pub fn render_table(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }

    let mut out = String::new();
    let header = format!("  ZACXIOM — {title}");

    out.push_str(&format!("┌{:─^WIDE$}┐\n", ""));
    out.push_str(&format!("│ {:<WIDE$} │\n", header));
    out.push_str(&format!("├{:─^WIDE$}├\n", ""));
    out.push_str(&format!(
        "│ {:<40} {:>6} {:>10} {:>15} │\n",
        "FILE", "SIZE", "RISK", "STATUS"
    ));
    out.push_str(&format!(
        "│{:─<40} {:─>6} {:─>10} {:─>15} │\n",
        "", "", "", ""
    ));

    for f in files.iter().take(50) {
        let fname = truncate_path(&f.path, 38);
        let risk_pct = format!("{:.0}%", f.risk_score * 100.0);
        let status = status_label(&f.decision);
        let size = human_size(f.size);

        out.push_str(&format!(
            "│ {:<40} {:>6} {:>10} {:>15} │\n",
            fname, size, risk_pct, status
        ));
    }

    if files.len() > 50 {
        out.push_str(&format!(
            "│ {:>76} │\n",
            format!("... and {} more files", files.len() - 50)
        ));
    }

    out.push_str(&format!("└{:─^WIDE$}┘\n", ""));
    out
}

/// Render the simulation report as a structured table.
pub fn render_simulation(files: &[ClassifiedFile], title: &str) -> String {
    if files.is_empty() {
        return format!("No files found for: {title}\n");
    }

    let mut out = String::new();
    let header = format!("  ZACXIOM SIMULATION — {title}");

    out.push_str(&format!("┌{:─^WIDE$}┐\n", ""));
    out.push_str(&format!("│ {:<WIDE$} │\n", header));
    out.push_str(&format!("├{:─^WIDE$}├\n", ""));
    out.push_str(&format!(
        "│ {:<35} {:>6} {:>12} {:>18} │\n",
        "FILE", "SIZE", "RISK", "ACTION"
    ));
    out.push_str(&format!(
        "│{:─<35} {:─>6} {:─>12} {:─>18} │\n",
        "", "", "", ""
    ));

    for f in files.iter().take(50) {
        let fname = truncate_path(&f.path, 33);
        let risk = format!("{:.0}%", f.risk_score * 100.0);
        let action = action_label(&f.decision);
        let size = human_size(f.size);

        out.push_str(&format!(
            "│ {:<35} {:>6} {:>12} {:>18} │\n",
            fname, size, risk, action
        ));
    }

    if files.len() > 50 {
        out.push_str(&format!(
            "│ {:>76} │\n",
            format!("... and {} more files", files.len() - 50)
        ));
    }

    out.push_str(&format!("└{:─^WIDE$}┘\n", ""));
    out
}

/// Context for rendering insights.
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

/// Render an insight footer after a report.
pub fn render_insights(ctx: &InsightContext) -> String {
    let safe_pct = if ctx.total > 0 {
        (ctx.safe as f64 / ctx.total as f64) * 100.0
    } else {
        0.0
    };
    let reclaimable = files_reclaimable_size(ctx.safe, ctx.low_risk, ctx.total_size);

    let mut out = String::new();
    out.push_str(&format!("{SEP:─>WIDE$}\n"));
    out.push_str("  INSIGHT\n");
    out.push_str(&format!("{SEP:─>WIDE$}\n"));

    out.push_str(&format!(
        "  {:.0}% of cache is safe to clean ({})\n",
        safe_pct,
        human_size(reclaimable)
    ));

    if ctx.open_files > 0 {
        out.push_str(&format!(
            "  {} files held open by running processes\n",
            ctx.open_files
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
    out.push_str(&format!("  Risk level: {risk_level}\n"));

    if ctx.protected > 0 {
        out.push_str(&format!(
            "  {} system-protected files excluded\n",
            ctx.protected
        ));
    }

    out.push_str(&format!("{SEP:─>WIDE$}\n"));
    out
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
        Decision::LowRisk => "WOULD CLEAN --smart",
        Decision::Moderate => "NEEDS --force",
        Decision::HighRisk => "BLOCKED",
        Decision::Protected => "NEVER",
    }
}

fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        return path.to_string();
    }
    // Show beginning and end: /home/user/.../file.ext
    let keep_start = max / 2;
    let keep_end = max - keep_start - 3;
    format!(
        "{}...{}",
        &path[..keep_start],
        &path[path.len() - keep_end..]
    )
}

fn files_reclaimable_size(safe: usize, low_risk: usize, total_size: u64) -> u64 {
    if safe + low_risk == 0 {
        return 0;
    }
    // Simple estimate: average file size * (safe + low_risk)
    let total_files = safe + low_risk;
    if total_files == 0 {
        return 0;
    }
    // proportional estimate from total
    (total_size as f64 * 0.6) as u64
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

    if unit_idx == 0 {
        format!("{:.0} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{CacheDomain, Ownership};

    fn make_file(path: &str, size: u64, decision: Decision) -> ClassifiedFile {
        ClassifiedFile {
            path: path.into(),
            size,
            cache_domain: CacheDomain::Browser,
            ownership: Ownership::User { uid: 1000 },
            risk_score: 0.0,
            risk_reasons: vec!["test".into()],
            decision,
        }
    }

    #[test]
    fn test_table_renders() {
        let files = vec![
            make_file("/home/user/.cache/test/a", 1024, Decision::Safe),
            make_file("/tmp/locked", 512, Decision::HighRisk),
        ];
        let out = render_table(&files, "Scan Result");
        assert!(out.contains("ELIGIBLE"));
        assert!(out.contains("BLOCKED"));
        assert!(out.contains("ZACXIOM"));
    }

    #[test]
    fn test_empty_table() {
        let out = render_table(&[], "Empty");
        assert!(out.contains("No files"));
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
        assert!(out.contains("INSIGHT"));
        assert!(out.contains("safe to clean"));
        assert!(out.to_lowercase().contains("risk level"));
    }

    #[test]
    fn test_truncate_path() {
        let long = "/home/user/very/long/path/that/exceeds/the/maximum/limit/file.txt";
        let truncated = truncate_path(long, 30);
        assert!(truncated.len() <= 30);
        assert!(truncated.contains("..."));
    }
}
