// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Advisor — Phase 7: Rendering.

use crate::color;
use crate::discovery::Ecosystem;
use crate::display::human_size;

use super::types::CleanupAdvisor;

/// Render star rating as Unicode stars.
pub(crate) fn star_rating(stars: u8) -> String {
    let filled = stars as usize;
    let empty = 5_usize.saturating_sub(filled);
    format!("{}{}", "\u{2605}".repeat(filled), "\u{2606}".repeat(empty))
}

/// Circled number labels for recommended order.
pub(crate) fn circled_number(n: usize) -> &'static str {
    match n {
        1 => "\u{2460}",
        2 => "\u{2461}",
        3 => "\u{2462}",
        4 => "\u{2463}",
        5 => "\u{2464}",
        6 => "\u{2465}",
        7 => "\u{2466}",
        8 => "\u{2467}",
        9 => "\u{2468}",
        10 => "\u{2469}",
        _ => "\u{2460}",
    }
}

/// Ecosystem-specific regeneration label (v8.5.1: P5).
///
/// Replaces the generic "Rebuild" label with wording appropriate
/// for each ecosystem.
pub(crate) fn ecosystem_regen_label(ecosystem: Option<Ecosystem>) -> &'static str {
    match ecosystem {
        Some(Ecosystem::Rust) => "Rebuild",
        Some(Ecosystem::Node) => "Reinstall time",
        Some(Ecosystem::Python) => "Environment setup",
        Some(Ecosystem::Go) => "Recompile",
        None => "Rebuild",
    }
}

/// Render the full advisor output.
///
/// v8.5: Grouped, action-first, decision-centric output.
/// Every recommendation is justified with explainable reasoning.
pub fn render_advisor(advisor: &CleanupAdvisor, _root: &std::path::Path) -> String {
    if advisor.opportunities.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // ── Header ──
    out.push_str(&color::section_header("PROJECT CLEANUP ADVISOR"));

    out.push_str(&format!("  {:<22} {}\n", "Project:", advisor.project_name));
    if let Some(eco) = advisor.ecosystem {
        out.push_str(&format!("  {:<22} {}\n", "Ecosystem:", eco.display()));
    }
    out.push('\n');

    // ── Opportunity Summary (P5: expanded) ──
    out.push_str(&color::section_header("SUMMARY"));

    let safe_count = advisor
        .opportunities
        .iter()
        .filter(|o| o.safe_to_clean)
        .count();

    let reclaim_pct = if advisor.directory_size > 0 {
        advisor.total_reclaimable as f64 / advisor.directory_size as f64 * 100.0
    } else {
        0.0
    };

    out.push_str(&format!(
        "  {:<22} {}\n",
        "Project size:",
        human_size(advisor.directory_size)
    ));
    out.push_str(&format!(
        "  {:<22} {} ({:.0}% of project)\n",
        "Estimated reclaim:",
        human_size(advisor.total_reclaimable),
        reclaim_pct
    ));

    // Largest reclaimable artifact
    if let Some(largest) = advisor.opportunities.iter().max_by_key(|o| o.size_bytes) {
        out.push_str(&format!(
            "  {:<22} {}\n",
            "Largest artifact:", largest.display_name
        ));
    }

    // Highest priority group
    if let Some(top_group) = advisor.groups.first() {
        out.push_str(&format!(
            "  {:<22} {} ({})\n",
            "Highest priority:",
            top_group.label,
            top_group.priority_level.display()
        ));
    }

    // Overall recommendation
    if let Some(top_group) = advisor.groups.first() {
        out.push_str(&format!(
            "  {:<22} {}\n",
            "Recommended action:", top_group.action
        ));
    }

    // Expected regeneration impact (P5: ecosystem-specific label)
    if let Some(slowest) = advisor
        .groups
        .iter()
        .max_by_key(|g| g.execution.regeneration_time.len())
    {
        let regen_label = format!("{} impact:", ecosystem_regen_label(advisor.ecosystem));
        out.push_str(&format!(
            "  {:<22} {}\n",
            regen_label, slowest.execution.regeneration_time
        ));
    }

    out.push_str(&format!(
        "  {:<22} {} safe operation{}\n",
        "Safe operations:",
        safe_count,
        if safe_count != 1 { "s" } else { "" }
    ));

    out.push('\n');

    // ── Recommendation Cards (P1, P2: grouped, action-first) ──
    out.push_str(&color::section_header("RECOMMENDATIONS"));

    for (i, group) in advisor.groups.iter().enumerate() {
        out.push('\n');

        // Value card header: priority + label
        let label = circled_number(i + 1);
        out.push_str(&format!(
            "  {} {} {}\n",
            label,
            star_rating(group.priority.stars()),
            group.label
        ));

        // Action (primary — most important line)
        out.push_str(&format!("     Action:     {}\n", group.action));

        // Size
        out.push_str(&format!(
            "     Reclaim:    {}\n",
            human_size(group.total_size)
        ));

        // Execution cost (P6: cleanup time)
        out.push_str(&format!(
            "     Cleanup:    {}\n",
            group.execution.cleanup_time
        ));

        // Regeneration time (P5: ecosystem-specific label)
        let regen_label = ecosystem_regen_label(advisor.ecosystem);
        out.push_str(&format!(
            "     {:<12}{}\n",
            format!("{}:", regen_label),
            group.execution.regeneration_time
        ));

        // Risk: always Verified Safe for items shown (planner guarantees this)
        out.push_str(&format!(
            "     Risk:       {}\n",
            color::purple("Verified Safe")
        ));

        // Confidence
        out.push_str(&format!("     Confidence: {}%\n", group.confidence_pct));

        // Items in group
        if group.items.len() > 1 {
            out.push_str("     Includes:   ");
            out.push_str(&group.items.join(", "));
            out.push('\n');
        }

        // Why this group (P9: explainability)
        if !group.reasons.is_empty() {
            out.push_str("     Why:        ");
            out.push_str(&group.reasons[0]);
            out.push('\n');
        }
    }

    out.push('\n');

    // ── Why This Order? (P4: explain ranking) ──
    if !advisor.groups.is_empty() {
        out.push_str(&color::section_header("WHY THIS ORDER?"));

        for (i, group) in advisor.groups.iter().enumerate() {
            out.push('\n');
            let label = circled_number(i + 1);
            out.push_str(&format!(
                "  {} {} — {}\n",
                label,
                group.label,
                group.priority_level.display()
            ));
            for reason in &group.ranking_reasons {
                out.push_str(&format!("  \u{2713} {reason}\n"));
            }
        }

        out.push('\n');
    }

    // ── Recommended Cleanup Order ──
    out.push_str(&color::section_header("EXECUTION PLAN"));

    for (i, group) in advisor.groups.iter().enumerate() {
        let label = circled_number(i + 1);
        out.push_str(&format!("  {} {}\n", label, group.action));
        let regen_key = ecosystem_regen_label(advisor.ecosystem).to_lowercase();
        out.push_str(&format!(
            "     {}  {}  {}: {}\n",
            human_size(group.total_size),
            group.priority_level.display(),
            regen_key,
            group.execution.regeneration_time
        ));
    }

    out.push('\n');
    out.push_str("  Source code is NEVER recommended for cleanup.\n");

    // v10: Evidence summary for auditability
    if let Some(eco) = advisor.ecosystem {
        if eco == crate::discovery::Ecosystem::Rust {
            let evidence_report =
                crate::evidence::collect_rust_evidence(_root, advisor.directory_size);
            out.push('\n');
            out.push_str(&color::section_header("EVIDENCE"));
            for item in &evidence_report.evidence.items {
                if item.passed {
                    out.push_str(&format!("  {} {}\n", item.icon(), item.title));
                }
            }
            out.push_str(&format!(
                "\n  Confidence: {}%  No hidden scoring.\n",
                evidence_report.confidence.final_score
            ));
        }
    }

    out
}
