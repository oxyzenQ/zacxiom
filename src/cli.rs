// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI definitions using clap derive.
//!
//! v6.2.0: added `explain` command and `--dry-run` flag.
//! v8.3.0: added `plan` command — cleanup recommendation engine.
//! v13.0.0: added `--exclude`, `--yes`, `--testconf`, `config` subcommand.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "zacxiom",
    version,
    about = "Safe filesystem cleaning, explained.",
    long_about = "Safe filesystem cleaning, explained.\n\
                  Clean safely. Explain every decision. Recover anything.\n\n\
                  Confidence tiers: ★★★★★ Maximum  ★★★★ High  ★★★ Moderate  ★★ Low  ★ Minimal\n\n\
                  v13: User-controlled safety — --exclude, config.toml, --testconf.",
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

    /// Validate config file and exit (does not run any command)
    #[arg(long, global = true)]
    pub testconf: bool,
}

#[derive(Subcommand)]
#[command(
    after_help = "💡 Quick start: zacxiom scan → zacxiom plan → zacxiom clean\n   All destructive commands are recoverable with zacxiom undo\n   \
                  v13: Use --exclude to protect specific paths/patterns"
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
        #[arg(num_args = 0..)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'm', long, default_value = "1")]
        min_size: u64,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
        /// Exclude paths/patterns from scan (e.g. --exclude "~/Downloads" --exclude "*.iso")
        /// Can be specified multiple times. Also read from config.toml [scan].exclude.
        #[arg(long)]
        exclude: Vec<String>,
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
    /// Safety levels (v13: confirmation required for smart/force unless --yes):
    ///   clean          — SAFE files only (default dry-run on first use)
    ///   clean --smart  — SAFE + LOW_RISK (requires confirmation or --yes)
    ///   clean --force  — SAFE + LOW + MODERATE (requires typing "DELETE" or --yes)
    ///
    /// v13: --force NO LONGER allows HighRisk files — those need manual `rm`.
    ///
    /// Examples:
    ///   zacxiom clean                    # conservative (dry-run on first use)
    ///   zacxiom clean --yes              # actually delete (skip dry-run + prompts)
    ///   zacxiom clean --smart --yes      # smart mode, auto-confirm
    ///   zacxiom clean --force --yes      # maximum cleanup, auto-confirm
    ///   zacxiom clean --exclude "~/Downloads"  # protect Downloads
    ///   zacxiom clean --dry-run --json   # preview as JSON
    Clean {
        #[arg(short = 'P', long, num_args = 0..)]
        paths: Vec<String>,
        /// Positional paths (alternative: zacxiom clean ~/.cache)
        #[arg(num_args = 0..)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        /// Also clean LOW_RISK files (caches, build artifacts)
        #[arg(long)]
        smart: bool,
        /// Also clean MODERATE files (requires confirmation unless --yes)
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
        /// Exclude paths/patterns from cleaning (e.g. --exclude "~/Downloads" --exclude "*.iso")
        #[arg(long)]
        exclude: Vec<String>,
        /// v13: Only clean files matching these patterns (whitelist mode).
        /// Example: --include "target/*" --include "node_modules/*"
        #[arg(long)]
        include: Vec<String>,
        /// v13: Stop on first error instead of continuing.
        #[arg(long)]
        fail_fast: bool,
        /// Auto-confirm all prompts (skip dry-run, skip confirmation)
        /// Required for non-interactive use (CI/scripts)
        #[arg(long)]
        yes: bool,
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
        #[arg(num_args = 0..)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
        /// Exclude paths/patterns from simulation
        #[arg(long)]
        exclude: Vec<String>,
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
        #[arg(num_args = 0..)]
        positional_paths: Vec<String>,
        #[arg(short, long, default_value = "0")]
        depth: usize,
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,
        #[arg(long)]
        json: bool,
        /// Exclude paths/patterns from report
        #[arg(long)]
        exclude: Vec<String>,
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

    /// Manage user configuration (v13)
    ///
    /// Zacxiom reads ~/.config/zacxiom/config.toml on startup.
    /// If the file has syntax errors or invalid values, zacxiom refuses to run.
    ///
    /// Usage:
    ///   zacxiom config init      # create default config
    ///   zacxiom config show      # print effective config
    ///   zacxiom config path      # print config file location
    ///   zacxiom config testconf  # validate config (same as --testconf)
    Config {
        #[command(subcommand)]
        action: ConfigAction,
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

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Create a default config file at ~/.config/zacxiom/config.toml
    Init,
    /// Print the effective configuration (config file merged with defaults)
    Show,
    /// Print the path to the config file
    Path,
    /// Validate the config file (alias for --testconf)
    Testconf,
}
