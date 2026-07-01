// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom undo` — snapshot restore command.

use crate::snapshot;

pub fn run_undo(id: Option<String>, list_only: bool) {
    let all = snapshot::Snapshot::list_all();

    if all.is_empty() {
        eprintln!("No snapshots found. Nothing to undo.");
        std::process::exit(1);
    }

    // List mode — show all snapshots
    if list_only {
        println!("Snapshots (newest first):");
        for (i, snap) in all.iter().enumerate() {
            let info = snapshot::Snapshot::load(snap).ok();
            let files = info.as_ref().map(|s| s.entry_count()).unwrap_or(0);
            let skipped = info.as_ref().map(|s| s.skipped_count()).unwrap_or(0);
            let date = info
                .as_ref()
                .and_then(|s| s.created())
                .unwrap_or("unknown".to_string());
            println!(
                "  {}. {}  {} files ({} skipped)  {}",
                i + 1,
                snap,
                files,
                skipped,
                date
            );
        }
        if all.len() == 1 {
            println!("\n  Run: zacxiom undo");
        } else {
            println!("\n  Restore latest:     zacxiom undo");
            println!(
                "  Restore specific:   zacxiom undo --id {}",
                all.first().unwrap()
            );
        }
        return;
    }

    let snap_id = match id {
        Some(ref i) => i.clone(),
        None => {
            if all.len() > 1 {
                let latest = all.first().unwrap();
                let count = snapshot::Snapshot::load(latest)
                    .map(|s| s.entry_count())
                    .unwrap_or(0);
                eprintln!(
                    "Multiple snapshots ({}). Restoring the latest: {} ({} files).",
                    all.len(),
                    latest,
                    count
                );
                eprintln!("Use --list to browse. Use --id to pick a specific one.");
            }
            all.first().unwrap().clone()
        }
    };

    println!("Restoring from snapshot: {snap_id}");
    match snapshot::Snapshot::load(&snap_id) {
        Ok(snap) => {
            let total = snap.entry_count();
            let skipped = snap.skipped_count();
            match snap.restore() {
                Ok(0) => {
                    if total == 0 && skipped > 0 {
                        eprintln!("Nothing to restore — all {skipped} entries were skipped (never removed).");
                    } else {
                        eprintln!("Nothing to restore — trash files may have been already restored or cleaned.");
                        eprintln!("Run 'zacxiom status' or 'zacxiom undo --list' to check available snapshots.");
                    }
                }
                Ok(n) => {
                    // v13.2: Learning risk model — flag restored paths so zacxiom
                    // learns that user wanted these files back. Future scans will
                    // raise risk for these paths, making them less likely to be
                    // auto-cleaned again.
                    let mut memory = crate::memory::ContextMemory::load();
                    for entry in &snap.entries {
                        if !entry.skipped {
                            memory.flag_path(
                                &entry.path,
                                "user restored via undo — avoid auto-cleaning",
                            );
                        }
                    }
                    // v13.3: Audit log
                    crate::audit::AuditEntry::undo(&snap_id, n).log();
                    if skipped > 0 {
                        println!("Restored {n} files ({} skipped — never removed).", skipped);
                    } else {
                        println!("Restored {n} files.");
                    }
                    println!(
                        "  💡 Risk model updated — these paths will be treated more cautiously."
                    );
                }
                Err(e) => {
                    eprintln!("Restore error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to load snapshot: {e}");
            std::process::exit(1);
        }
    }
}
