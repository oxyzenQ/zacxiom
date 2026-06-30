// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom config` — manage user configuration.

use crate::config;

/// `zacxiom config init` — create a default config file.
pub fn run_config_init() {
    match config::write_default_config() {
        Ok(path) => {
            println!("Created default config at: {}", path.display());
            println!();
            println!("Edit it to customize zacxiom's behavior:");
            println!("  nano {}", path.display());
            println!();
            println!("Validate your changes with:");
            println!("  zacxiom --testconf");
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

/// `zacxiom config show` — print the effective configuration.
pub fn run_config_show(cfg: &config::Config) {
    println!("━━━ ZACXIOM CONFIGURATION ━━━");
    println!("  Config file: {}", config::config_path().display());
    let exists = config::config_path().exists();
    println!(
        "  Status: {}",
        if exists {
            "loaded"
        } else {
            "not found — using defaults"
        }
    );
    println!();
    println!("[scan]");
    println!("  exclude           : {:?}", cfg.scan.exclude);
    println!("  exclude_patterns  : {:?}", cfg.scan.exclude_patterns);
    println!("  min_size          : {} bytes", cfg.scan.min_size);
    println!("  warn_user_dirs    : {}", cfg.scan.warn_user_dirs);
    println!();
    println!("[clean]");
    println!(
        "  require_confirmation : {}",
        cfg.clean.require_confirmation
    );
    println!("  default_mode         : {}", cfg.clean.default_mode);
    println!(
        "  protect_extensions   : {:?}",
        cfg.clean.protect_extensions
    );
    println!("  protect_patterns     : {:?}", cfg.clean.protect_patterns);
    println!(
        "  max_auto_clean_size  : {} bytes ({})",
        cfg.clean.max_auto_clean_size,
        crate::simulator::human_size(cfg.clean.max_auto_clean_size)
    );
    println!("  first_run_dry_run    : {}", cfg.clean.first_run_dry_run);
    println!();
    println!("[snapshot]");
    println!("  dir              : {}", cfg.snapshot.dir);
    println!("  auto_prune_days  : {}", cfg.snapshot.auto_prune_days);
    println!();
    println!("[trash]");
    println!("  verify_checksum  : {}", cfg.trash.verify_checksum);
}

/// `zacxiom config path` — print the config file location.
pub fn run_config_path() {
    println!("{}", config::config_path().display());
}
