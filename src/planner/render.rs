// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — Plan rendering.

use std::path::Path;

use crate::color;
use crate::display::human_size;

use super::types::CleanupPlan;

/// Render a cleanup plan to a formatted string matching Zacxiom visual style.
pub fn render_plan(plan: &CleanupPlan, path: &Path) -> String {
    let mut out = String::new();

    out.push_str(&color::section_header("CLEANUP PLAN"));

    let displaypath = if path.to_string_lossy().ends_with('/') {
        path.to_string_lossy().to_string()
    } else {
        format!("{}/", path.display())
    };
    out.push_str(&format!("  {:<20} {}\n", "Target:", displaypath));
    out.push_str(&format!(
        "  {:<20} {}\n",
        "Risk:",
        plan.risk_level.display()
    ));
    out.push_str(&format!(
        "  {:<20} {}\n",
        "Space:",
        human_size(plan.estimated_reclaimable_bytes)
    ));

    let safe_label = if plan.safe_to_clean {
        color::purple("YES")
    } else {
        "NO".to_string()
    };
    out.push_str(&format!("  {:<20} {}\n", "Safe To Clean:", safe_label));
    out.push('\n');

    if !plan.recommendation.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Recommendation:", plan.recommendation
        ));
    }
    if !plan.reason.is_empty() {
        out.push_str(&format!("  {:<20} {}\n", "Reason:", plan.reason));
    }
    if !plan.regeneration.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Regeneration:", plan.regeneration
        ));
    }
    if !plan.suggested_commands.is_empty() {
        out.push_str("  Suggested Commands:\n");
        for cmd in &plan.suggested_commands {
            out.push_str(&format!("    {}\n", cmd));
        }
    }

    if !plan.expected_result.is_empty() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            "Expected Result:", plan.expected_result
        ));
    }

    if !plan.safer_alternatives.is_empty() {
        out.push('\n');
        out.push_str("  Consider reviewing:\n");
        for alt in &plan.safer_alternatives {
            out.push_str(&format!("    - {}\n", alt));
        }
    }

    if !plan.notes.is_empty() {
        out.push('\n');
        for note in &plan.notes {
            out.push_str(&format!("  {}\n", note));
        }
    }

    out
}
