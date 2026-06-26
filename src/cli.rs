// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI definitions using clap derive.
//!
//! v6.2.0: added `explain` command and `--dry-run` flag.
//! v8.3.0: added `plan` command — cleanup recommendation engine.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "zacxiom",
    version,
    about = "Filesystem Intelligence Engine — Observe → Understand → Decide → Act",
    long_about = "Safe-by-default filesystem cleanup with full explainability.\n\
                  Every decision is justified. Every action is logged.",
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Print version information
    #[arg(short = 'V', long, global = true)]
    pub version: bool,

    /// Check for latest upstream release
    #[arg(long, global = true)]
    pub check_update: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan filesystem for cache files and classify them
    Scan {
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'm', long, default_value = "1")]
        min_size: u64,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
    },

    /// Show full classified report
    Report {
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
    },

    /// Dry-run simulation — see what WOULD happen
    Simulate {
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
    },

    /// Execute safe clean (only SAFE files unless --smart/--force)
    Clean {
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        /// Also clean LOW_RISK files
        #[arg(long)]
        smart: bool,
        /// Also clean MODERATE files
        #[arg(long)]
        force: bool,
        /// Preview only — show what WOULD be cleaned without deleting
        #[arg(long)]
        dry_run: bool,
        /// Show individual file list (default: summary only)
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
    },

    /// Explain why a file or domain is safe/risky (★★★★★ trust cards)
    ///
    /// Usage: zacxiom explain ~/.cargo
    ///        zacxiom explain ~/.rustup
    Explain {
        /// File path or domain name to explain
        path: String,
    },

    Undo {
        #[arg(short, long)]
        id: Option<String>,
    },
    Status,

    /// Plan cleanup — what is safe and recommended? (read-only, never deletes)
    ///
    /// Usage: zacxiom plan ~/.cache
    ///        zacxiom plan target
    ///        zacxiom plan node_modules
    Plan {
        /// Path to plan cleanup for
        path: String,
    },

    /// Analyze unknown files — what dominates the Unknown bucket?
    InspectUnknown {
        /// Scan root(s), defaults to auto-detect
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        /// JSON export for analysis
        #[arg(long)]
        json: bool,
        /// Show near-miss classifications (debugging)
        #[arg(long)]
        verbose: bool,
    },

    /// Check for latest upstream release
    ///
    /// Usage: zacxiom check-update
    CheckUpdate,

    /// Show complete confidence calculation for a path
    ///
    /// Usage: zacxiom explain-confidence ~/projects/foo
    ExplainConfidence {
        /// Path to analyze
        path: String,
    },

    /// Show detailed risk reasoning for a path
    ///
    /// Usage: zacxiom explain-risk ~/projects/foo
    ExplainRisk {
        /// Path to analyze
        path: String,
    },
}
