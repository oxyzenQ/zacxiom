// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom doctor` — system health and readiness check.

use crate::snapshot;
use std::path::{Path, PathBuf};

pub fn run_doctor(golden: bool) {
    let version = if golden {
        "<VERSION>".to_string()
    } else {
        format!("v{}", env!("CARGO_PKG_VERSION"))
    };
    println!("zacxiom {} system check", version);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut ok = 0;
    let mut warn = 0;
    let mut fail = 0;

    macro_rules! check {
        ($label:expr, $cond:expr, $detail:expr) => {{
            if $cond {
                println!("  ✓  {}", $label);
                ok += 1;
            } else {
                println!("  ✗  {} — {}", $label, $detail);
                fail += 1;
            }
        }};
    }

    macro_rules! check_warn {
        ($label:expr, $cond:expr, $detail:expr) => {{
            if $cond {
                println!("  ✓  {}", $label);
                ok += 1;
            } else {
                println!("  ⚠  {} — {} (non-blocking)", $label, $detail);
                warn += 1;
            }
        }};
    }

    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into()));

    // Core checks
    check!("Rust runtime", true, ""); // executing means Rust works

    // Config
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("zacxiom");
    check_warn!(
        "Config directory",
        config_dir.exists() || config_dir.parent().is_some_and(|p| p.exists()),
        "run 'zacxiom scan' to initialize"
    );

    // Cache
    let cache_dir = snapshot::snapshot_dir();
    check!(
        "Cache directory writable",
        test_writable(&cache_dir),
        &format!("cannot write to {}", cache_dir.display())
    );

    // Trash
    let trash_dir = snapshot::trash_dir();
    check!(
        "Trash directory writable",
        test_writable(&trash_dir),
        &format!("cannot write to {}", trash_dir.display())
    );

    // Snapshots — distinguish: no dir, dir unreadable, empty dir, has snaps
    let snap_dir = snapshot::snapshot_dir();
    let snaps = if snap_dir.exists() {
        match std::fs::read_dir(&snap_dir) {
            Ok(_) => snapshot::Snapshot::list_all(),
            Err(e) => {
                check!(
                    "Snapshots accessible",
                    false,
                    &format!("cannot access {} ({})", snap_dir.display(), e)
                );
                vec![]
            }
        }
    } else {
        vec![]
    };
    if snap_dir.exists() && snaps.is_empty() && fail == 0 {
        // dir exists and is readable, but no snap files — normal fresh install
        check_warn!(
            "Snapshots present",
            false,
            "no snapshots yet (normal on fresh install)"
        );
    } else if !snaps.is_empty() {
        check_warn!("Snapshots present", true, "");
    }

    // Home access
    let test_paths = [
        home.join(".cache"),
        home.join(".local/share"),
        Path::new("/tmp").to_path_buf(),
    ];
    let accessible = test_paths.iter().any(|p| p.exists());
    check_warn!(
        "Common cache paths accessible",
        accessible,
        "some cache directories not found (may be normal)"
    );

    // Binary
    let binary = std::env::current_exe().unwrap_or_default();
    check!(
        "Binary accessible",
        binary.exists(),
        &format!("binary not found at {}", binary.display())
    );

    println!("━━━━━━━━━━━━━━━━━━━━━━━━");
    if fail > 0 {
        println!("  {} passed, {} warnings, {} failures", ok, warn, fail);
        println!("  ⚠  System not fully ready. Fix failures above.");
        std::process::exit(1);
    } else if warn > 0 {
        println!("  {} passed, {} warnings (non-blocking)", ok, warn);
        println!("  ✓  System ready.");
    } else {
        println!("  {} passed. System ready.", ok);
    }
}

fn test_writable(dir: &Path) -> bool {
    if dir.exists() {
        // If it exists, verify it's a writable directory
        let test_file = dir.join(".zacxiom_write_test");
        match std::fs::write(&test_file, b"test") {
            Ok(()) => {
                let _ = std::fs::remove_file(&test_file);
                true
            }
            Err(_) => false,
        }
    } else {
        // If it doesn't exist, check if parent is writable
        dir.parent().is_some_and(test_writable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_smoke() {
        // Just verify it doesn't panic
        run_doctor(false);
    }
}
