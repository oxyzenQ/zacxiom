// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI definitions using clap derive.
//!
//! Commands: scan, report, simulate, clean, check-update
//! Flags: --smart, --force, --json, --depth, --min-size

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "zacxiom",
    version, // overridden by custom --version in main()
    about = "Filesystem Intelligence Engine — Observe → Understand → Decide → Act",
    long_about = "Safe-by-default filesystem cleanup with full explainability.\n\
                  Every decision is justified. Every action is logged.\n\
                  Run `simulate` before `clean` — always.",
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
        /// Root paths to scan (default: ~/.cache, /var/cache, /tmp)
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,

        /// Maximum directory depth (0 = unlimited)
        #[arg(short, long, default_value = "0")]
        depth: usize,

        /// Minimum file size in bytes
        #[arg(short = 'm', long, default_value = "1")]
        min_size: u64,

        /// Profile: minimal, dev, gaming, server
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show full classified report (alias: scan with verbose)
    Report {
        /// Root paths to scan
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,

        /// Maximum directory depth (0 = unlimited)
        #[arg(short, long, default_value = "0")]
        depth: usize,

        /// Profile: minimal, dev, gaming, server
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Dry-run simulation — see what WOULD happen (MANDATORY before clean)
    Simulate {
        /// Root paths to scan
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,

        /// Maximum directory depth (0 = unlimited)
        #[arg(short, long, default_value = "0")]
        depth: usize,

        /// Profile: minimal, dev, gaming, server
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Execute safe clean (only SAFE files unless --smart/--force)
    Clean {
        /// Root paths to scan
        #[arg(short, long, num_args = 0..)]
        paths: Vec<String>,

        /// Maximum directory depth (0 = unlimited)
        #[arg(short, long, default_value = "0")]
        depth: usize,

        /// Profile: minimal, dev, gaming, server
        #[arg(short = 'p', long, default_value = "dev")]
        profile: String,

        /// Also clean LOW_RISK files
        #[arg(long)]
        smart: bool,

        /// Also clean MODERATE files (requires confirmation)
        #[arg(long)]
        force: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Check for latest upstream release on GitHub
    CheckUpdate,
}
