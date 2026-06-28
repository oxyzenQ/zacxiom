// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom status` — system health and state command.

use crate::history;
use crate::memory;
use crate::policy;
use crate::profiles;
use crate::safety;
use crate::snapshot;

pub fn run_status(golden: bool) {
    let health = profiles::detect_health();
    let hist = history::History::load();
    let snaps = snapshot::Snapshot::list_all();
    let policy = policy::Policy::load();
    let mem = memory::ContextMemory::load();
    let safe = safety::system_health_check();

    let version = if golden {
        "<VERSION>".to_string()
    } else {
        format!("v{}", env!("CARGO_PKG_VERSION"))
    };

    println!("────────────");
    println!("  ZACXIOM {} STATUS", version);
    println!("────────────");
    println!("  Health    : {:?}", health);
    if golden {
        println!("  History   : <NUM> records");
        println!("  Snapshots : <NUM> available");
        println!("  Memory    : <NUM> sessions, <NUM> trusted, <NUM> flagged");
        println!("  Stability : <STABILITY>");
    } else {
        println!("  History   : {} records", hist.records.len());
        println!("  Snapshots : {} available", snaps.len());
        println!(
            "  Memory    : {} sessions, {} trusted, {} flagged",
            mem.sessions,
            mem.trusted_paths.len(),
            mem.flagged_paths.len()
        );
        println!(
            "  Stability : {}",
            if mem.is_stabilized() {
                "stabilized"
            } else {
                "learning"
            }
        );
    }
    if !policy.protected_paths.is_empty() {
        println!(
            "  Policy    : {} user-protected paths",
            policy.protected_paths.len()
        );
    }
    // v11: Snapshot storage reporting
    let (snap_count, snap_total_size) = snapshot::total_snapshot_size();
    if golden {
        println!("  Snapshots : <NUM>");
        println!("  Disk usage: <SIZE>");
    } else {
        println!("  Snapshots : {snap_count}");
        if snap_count > 0 {
            println!(
                "  Disk usage: {}",
                crate::simulator::human_size(snap_total_size)
            );
        }
    }

    if !snaps.is_empty() {
        if golden {
            println!("  Last snap : <SNAP-ID>");
        } else {
            println!("  Last snap : {}", snaps.first().unwrap());
        }
    }
    // Show most recent clean action - masked in golden mode
    if golden {
        // Don't show dynamic last-clean in golden mode
    } else if let Some(last_clean) = hist
        .records
        .iter()
        .filter(|r| r.action == "clean")
        .max_by_key(|r| r.timestamp.clone())
    {
        println!(
            "  Last clean: {} files ({})",
            last_clean.files_removed,
            &last_clean.timestamp[..10]
        );
    }
    println!(
        "  Safety    : {}",
        if safe.passed { "PASS" } else { "FAIL" }
    );
    println!("────────────");
}
