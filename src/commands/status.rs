// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom status` — system health and state command.

use crate::history;
use crate::memory;
use crate::policy;
use crate::profiles;
use crate::safety;
use crate::snapshot;

pub fn run_status() {
    let health = profiles::detect_health();
    let hist = history::History::load();
    let snaps = snapshot::Snapshot::list_all();
    let policy = policy::Policy::load();
    let mem = memory::ContextMemory::load();
    let safe = safety::system_health_check();

    println!("────────────");
    println!("  ZACXIOM v{} STATUS", env!("CARGO_PKG_VERSION"));
    println!("────────────");
    println!("  Health    : {:?}", health);
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
    if !policy.protected_paths.is_empty() {
        println!(
            "  Policy    : {} user-protected paths",
            policy.protected_paths.len()
        );
    }
    if !snaps.is_empty() {
        println!("  Last snap : {}", snaps.last().unwrap());
    }
    println!(
        "  Safety    : {}",
        if safe.passed { "PASS" } else { "FAIL" }
    );
    println!("────────────");
}
