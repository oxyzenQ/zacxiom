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
                  Every decision is justified. Every action is logged.\n\n\
                  Confidence tiers: ★★★★★ Maximum  ★★★★ High  ★★★ Moderate  ★★ Low  ★ Minimal",
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
#[command(
    after_help = "💡 Quick start: zacxiom scan → zacxiom plan → zacxiom clean\n   All destructive commands are recoverable with zacxiom undo"
)]
pub enum Command {
    /// Scan filesystem for cache files and classify them
    ///
    /// Safe, read-only. The first step — discover what's on your system.
    Scan {
        /// Paths to scan (e.g. zacxiom scan ~/.cache ~/.npm)
        #[arg(short = 'P', long, num_args = 0..)]
        paths: Vec<String>,
        /// Positional paths (alternative: zacxiom scan ~/.cache)
        #[arg(num_args = 0.., trailing_var_arg = true)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'm', long, default_value = "1")]
        min_size: u64,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
    },

    /// Plan cleanup — what is safe and recommended? (read-only, never deletes)
    ///
    /// Usage: zacxiom plan              # defaults to HOME
    ///        zacxiom plan ~/.cache
    ///        zacxiom plan target
    ///        zacxiom plan node_modules
    Plan {
        /// Path to plan cleanup for (defaults to HOME)
        path: Option<String>,
    },

    /// Execute safe clean — removes files with trash-based recovery
    ///
    /// Safety levels:
    ///   clean          — SAFE files only (strict, recommended for beginners)
    ///   clean --smart  — SAFE + LOW_RISK (more aggressive, still recoverable)
    ///   clean --force  — SAFE + LOW + MODERATE (requires explicit YES confirmation)
    ///
    /// Examples:
    ///   zacxiom clean                    # conservative
    ///   zacxiom clean --smart            # recommended for experienced users
    ///   zacxiom clean --force            # maximum cleanup with confirmation
    ///   zacxiom clean --dry-run --json   # preview as JSON
    Clean {
        #[arg(short = 'P', long, num_args = 0..)]
        paths: Vec<String>,
        /// Positional paths (alternative: zacxiom clean ~/.cache)
        #[arg(num_args = 0.., trailing_var_arg = true)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        /// Also clean LOW_RISK files (caches, build artifacts)
        #[arg(long)]
        smart: bool,
        /// Also clean MODERATE files (requires YES confirmation)
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

    /// Restore files from a cleanup snapshot
    ///
    /// Without --id, restores the latest snapshot.
    /// Usage: zacxiom undo --id snap-xxx
    ///        zacxiom undo --list
    Undo {
        /// Snapshot ID to restore (defaults to latest)
        #[arg(short, long)]
        id: Option<String>,
        /// List all available snapshots
        #[arg(short = 'l', long)]
        list: bool,
    },

    /// Show system status — health, history, snapshots, memory
    Status {
        /// Golden test mode — mask dynamic values for deterministic output
        #[arg(long, hide = true)]
        golden: bool,
    },

    /// Run system health check — verify config, permissions, readiness
    Doctor {
        /// Golden test mode — mask dynamic values for deterministic output
        #[arg(long, hide = true)]
        golden: bool,
    },

    /// Dry-run simulation — see what WOULD happen before running clean
    Simulate {
        #[arg(short = 'P', long, num_args = 0..)]
        paths: Vec<String>,
        /// Positional paths
        #[arg(num_args = 0.., trailing_var_arg = true)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
    },

    /// Explain why a path is safe/risky with ★★★★★ trust cards
    ///
    /// Usage: zacxiom explain <path>
    ///
    /// Examples:
    ///   zacxiom explain ~/.cargo
    ///   zacxiom explain ~/.rustup
    Explain {
        /// File path or domain name to explain
        path: String,
    },

    /// Show full classified report (same data as scan, more detail)
    Report {
        #[arg(short = 'P', long, num_args = 0..)]
        paths: Vec<String>,
        /// Positional paths
        #[arg(num_args = 0.., trailing_var_arg = true)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
    },

    /// Analyze unknown files — what dominates the Unknown bucket?
    InspectUnknown {
        /// Scan root(s), defaults to auto-detect
        #[arg(short = 'P', long, num_args = 0..)]
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

    /// Manage cleanup snapshots — list, delete, prune, purge
    ///
    /// Usage:
    ///   zacxiom snapshot list
    ///   zacxiom snapshot delete <id>
    ///   zacxiom snapshot prune --keep N
    ///   zacxiom snapshot prune --older-than 30d
    ///   zacxiom snapshot purge --confirm "DELETE ALL"
    Snapshot {
        #[command(subcommand)]
        action: Option<SnapshotAction>,
    },
}

#[derive(Subcommand)]
pub enum SnapshotAction {
    /// List all snapshots with size, creation date, and age
    List {
        #[arg(long)]
        json: bool,
    },
    /// Delete a snapshot by ID
    Delete {
        /// Snapshot ID to delete
        id: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
    /// Prune old snapshots
    Prune {
        /// Keep only the newest N snapshots
        #[arg(long)]
        keep: Option<usize>,
        /// Delete snapshots older than threshold (e.g. "30d", "7d", "24h")
        #[arg(long)]
        older_than: Option<String>,
    },
    /// Delete ALL snapshots permanently
    Purge {
        /// Confirmation string — must be exactly "DELETE ALL"
        #[arg(long)]
        confirm: Option<String>,
    },
}
