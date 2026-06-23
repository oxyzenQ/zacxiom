// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI definitions using clap derive.
//!
//! v6.2.0: added `explain` command and `--dry-run` flag.

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
    ///        zacxiom explain --path ~/.rustup
    Explain {
        /// File path or domain name to explain (positional)
        #[arg(default_value = "")]
        target: String,

        /// File path or domain name (named flag)
        #[arg(short, long)]
        path: Option<String>,
    },

    CheckUpdate,
    Undo {
        #[arg(short, long)]
        id: Option<String>,
    },
    Status,
}
