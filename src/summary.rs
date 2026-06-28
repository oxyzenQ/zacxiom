// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Decision summary layer — answers "What can I safely reclaim?" immediately.
//!
//! Renders before any detailed table. Single glance = full picture.

use crate::rules::{ClassifiedFile, Decision};

#[allow(dead_code)]
pub struct DecisionSummary {
    pub files_found: usize,
    pub safe_to_clean: usize,
    pub low_risk: usize,
    pub blocked: usize,
    pub protected: usize,
    pub recoverable_bytes: u64,
    pub total_bytes: u64,
    pub risk_level: String,
    pub open_files: usize,
}

impl DecisionSummary {
    pub fn from_files(files: &[ClassifiedFile], open_files: usize) -> Self {
        let mut safe = 0usize;
        let mut low = 0usize;
        let mut blocked = 0usize;
        let mut prot = 0usize;
        let mut recoverable = 0u64;
        let mut total = 0u64;

        for f in files {
            total += f.size;
            match f.decision {
                Decision::Safe => {
                    safe += 1;
                    recoverable += f.size;
                }
                Decision::LowRisk => {
                    low += 1;
                    recoverable += f.size;
                }
                Decision::HighRisk | Decision::Moderate => blocked += 1,
                Decision::Protected | Decision::ProtectedActiveEnvironment => prot += 1,
            }
        }

        let risk_level = if prot > files.len() / 3 {
            "HIGH — many protected files"
        } else if blocked > files.len() / 2 {
            "HIGH — most files blocked"
        } else if safe > files.len() / 2 {
            "LOW — majority safe"
        } else if low > 0 {
            "MEDIUM — some files require --smart"
        } else {
            "HIGH — review required"
        };

        DecisionSummary {
            files_found: files.len(),
            safe_to_clean: safe,
            low_risk: low,
            blocked,
            protected: prot,
            recoverable_bytes: recoverable,
            total_bytes: total,
            risk_level: risk_level.to_string(),
            open_files,
        }
    }
}
