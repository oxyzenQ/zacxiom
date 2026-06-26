// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom undo` — snapshot restore command.

use crate::snapshot;

pub fn run_undo(id: Option<String>) {
    let snap_id = match id {
        Some(ref i) => i.clone(),
        None => {
            let all = snapshot::Snapshot::list_all();
            if all.is_empty() {
                eprintln!("No snapshots found. Nothing to undo.");
                std::process::exit(1);
            }
            all.last().unwrap().clone()
        }
    };

    println!("Restoring from snapshot: {snap_id}");
    match snapshot::Snapshot::load(&snap_id) {
        Ok(snap) => match snap.restore() {
            Ok(n) => println!("Restored {n} files."),
            Err(e) => {
                eprintln!("Restore error: {e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Failed to load snapshot: {e}");
            std::process::exit(1);
        }
    }
}
