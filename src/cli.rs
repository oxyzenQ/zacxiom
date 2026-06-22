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
        #[arg(long)]
        json: bool,
    },

    /// Explain why a file or domain is safe/risky (★★★★★ trust cards)
    Explain {
        /// File path or domain name to explain
        #[arg(short, long)]
        path: String,
    },

    CheckUpdate,
    Undo {
        #[arg(short, long)]
        id: Option<String>,
    },
    Status,
}
