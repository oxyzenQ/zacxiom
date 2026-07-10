// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom snapshot` — Snapshot management commands (v11.0.0).
//!
//! Provides listing, deletion, pruning, and purging of cleanup snapshots.

use crate::snapshot;

/// List all snapshots with ID, size, creation date, and age.
pub fn run_snapshot_list(json: bool) {
    let all = snapshot::Snapshot::list_all();

    if all.is_empty() {
        if json {
            println!("{{\"snapshots\":[]}}");
        } else {
            println!("No snapshots found.");
        }
        return;
    }

    if json {
        let mut snapshots_json = Vec::new();
        for snap_id in &all {
            if let Ok(snap) = snapshot::Snapshot::load(snap_id) {
                let size: u64 = snap
                    .entries
                    .iter()
                    .filter(|e| !e.skipped)
                    .map(|e| e.size)
                    .sum();
                snapshots_json.push(serde_json::json!({
                    "id": snap.id,
                    "created": snap.created,
                    "entries": snap.entry_count(),
                    "skipped": snap.skipped_count(),
                    "size": size,
                }));
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"snapshots": snapshots_json}))
                .unwrap()
        );
        return;
    }

    let count = all.len();
    let mut total_size: u64 = 0;

    println!("Snapshots ({count}):");
    let age_header = "Age";
    println!(
        "{:<6} {:<20} {:<10} {:<8} {:<10} {age_header}",
        "ID", "Created", "Size", "Entries", "Skipped"
    );
    println!("{}", "─".repeat(70));

    for (i, snap_id) in all.iter().enumerate() {
        if let Ok(snap) = snapshot::Snapshot::load(snap_id) {
            let size: u64 = snap
                .entries
                .iter()
                .filter(|e| !e.skipped)
                .map(|e| e.size)
                .sum();
            total_size += size;
            let _created = snap.created().unwrap_or_else(|| "unknown".to_string());
            let age = snapshot::snapshot_age(snap_id);
            println!(
                "  {:<3}  {:<18}  {:>8}  {:>6}  {:>6}  {}",
                i + 1,
                format_snap_id(snap_id),
                crate::simulator::human_size(size),
                snap.entry_count(),
                snap.skipped_count(),
                age,
            );
        }
    }

    println!("{}", "─".repeat(70));
    println!(
        "  Total: {} across {count} snapshot(s)",
        crate::simulator::human_size(total_size)
    );
}

/// Delete a single snapshot by ID.
pub fn run_snapshot_delete(id: &str, force: bool) {
    if !force {
        let snap = match snapshot::Snapshot::load(id) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("Snapshot not found: {id}");
                eprintln!("Run 'zacxiom snapshot list' to see available snapshots.");
                std::process::exit(1);
            }
        };
        let count = snap.entry_count();
        println!("Snapshot: {id}");
        println!("  Entries: {count}");
        println!();
        println!("This will permanently delete the snapshot metadata.");
        println!("Trash files will NOT be deleted (use --purge-trash to also remove trash).");
        println!();
        println!("To confirm, re-run with --force");
        return;
    }

    match snapshot::Snapshot::delete(id) {
        Ok(()) => {
            println!("Snapshot {id} deleted.");
        }
        Err(e) => {
            eprintln!("Error deleting snapshot {id}: {e}");
            std::process::exit(1);
        }
    }
}

/// Prune snapshots, keeping only the newest N.
pub fn run_snapshot_prune_keep(keep: usize) {
    if keep == 0 {
        eprintln!("--keep must be at least 1");
        std::process::exit(1);
    }

    let all = snapshot::Snapshot::list_all();
    if all.len() <= keep {
        println!(
            "{} snapshot(s) found, keeping all (--keep {keep}). Nothing pruned.",
            all.len()
        );
        return;
    }

    let to_delete: Vec<_> = all.iter().skip(keep).collect();
    let mut deleted = 0;
    for snap_id in to_delete {
        match snapshot::Snapshot::delete(snap_id) {
            Ok(()) => {
                println!("  Deleted: {snap_id}");
                deleted += 1;
            }
            Err(e) => {
                eprintln!("  Error deleting {snap_id}: {e}");
            }
        }
    }

    println!("Pruned {deleted} snapshot(s) — kept newest {keep}.",);
}

/// Prune snapshots older than a given age string (e.g. "30d", "7d", "1h").
pub fn run_snapshot_prune_older_than(age_str: &str) {
    let max_age_secs = match parse_age(age_str) {
        Some(s) => s,
        None => {
            eprintln!("Invalid age format: {age_str}");
            eprintln!("Use format like: 30d, 7d, 24h, 1h");
            std::process::exit(1);
        }
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let all = snapshot::Snapshot::list_all();
    let mut deleted = 0;
    let mut kept = 0;

    for snap_id in &all {
        let snap_age = snapshot::snapshot_age_secs(snap_id);
        let age = now_secs.saturating_sub(snap_age);

        if age > max_age_secs {
            match snapshot::Snapshot::delete(snap_id) {
                Ok(()) => {
                    println!("  Deleted: {snap_id}");
                    deleted += 1;
                }
                Err(e) => {
                    eprintln!("  Error deleting {snap_id}: {e}");
                }
            }
        } else {
            kept += 1;
        }
    }

    println!("Pruned {deleted} snapshot(s) older than {age_str} — {kept} kept.",);
}

/// Purge ALL snapshots. Requires confirmation by typing "DELETE ALL".
pub fn run_snapshot_purge(confirmed: &str) {
    if confirmed != "DELETE ALL" {
        if confirmed.is_empty() {
            eprintln!("⚠️  WARNING: This will permanently delete ALL snapshots.");
            eprintln!("   Trash files will also be removed.");
            eprintln!();
            eprintln!("   To confirm, type:");
            eprintln!("     zacxiom snapshot purge --confirm \"DELETE ALL\"");
        } else {
            eprintln!("Invalid confirmation. To purge, type:");
            eprintln!("  zacxiom snapshot purge --confirm \"DELETE ALL\"");
        }
        std::process::exit(1);
    }

    let all = snapshot::Snapshot::list_all();
    if all.is_empty() {
        println!("No snapshots to purge.");
        return;
    }
    let total = all.len();

    // Delete metadata files from BOTH XDG and legacy directories.
    // list_all() aggregates from both dirs, so purge must delete from both.
    let mut deleted = 0;
    let mut failed = Vec::new();
    for snap_id in &all {
        match snapshot::delete_snapshot(snap_id) {
            Ok(()) => deleted += 1,
            Err(e) => failed.push(format!("{snap_id}: {e}")),
        }
    }

    // Also clean up trash directories (both XDG and legacy)
    for trash_path in &[snapshot::trash_dir(), legacy_trash_dir()] {
        if trash_path.exists() {
            let _ = std::fs::remove_dir_all(trash_path);
        }
    }

    if failed.is_empty() {
        println!("Purged ALL {deleted} snapshot(s).");
    } else {
        println!("Purged {deleted}/{total} snapshot(s).");
        for f in &failed {
            eprintln!("  Failed: {f}");
        }
    }
    println!("Trash directory cleared.");
}

// ── Helpers ────────────────────────────────────────────────────

/// Legacy trash directory (pre-v13, ~/.cache/zacxiom/trash).
fn legacy_trash_dir() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    std::path::PathBuf::from(home).join(".cache/zacxiom/trash")
}

fn format_snap_id(id: &str) -> String {
    if id.len() > 18 {
        format!("{}…", &id[..17])
    } else {
        id.to_string()
    }
}

/// Parse age string like "30d", "7d", "24h", "1h", "90s" into seconds.
fn parse_age(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: u64 = num_str.parse().ok()?;

    match unit {
        "s" => Some(num),
        "m" => Some(num * 60),
        "h" => Some(num * 60 * 60),
        "d" => Some(num * 24 * 60 * 60),
        "w" => Some(num * 7 * 24 * 60 * 60),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_age() {
        assert_eq!(parse_age("30d"), Some(30 * 86400));
        assert_eq!(parse_age("7d"), Some(7 * 86400));
        assert_eq!(parse_age("24h"), Some(24 * 3600));
        assert_eq!(parse_age("1h"), Some(3600));
        assert_eq!(parse_age("90s"), Some(90));
        assert_eq!(parse_age("2w"), Some(2 * 7 * 86400));
        assert_eq!(parse_age(""), None);
        assert_eq!(parse_age("abc"), None);
    }

    #[test]
    fn test_format_snap_id() {
        assert_eq!(
            format_snap_id("snap-12345678901234567890"),
            "snap-123456789012…"
        );
        assert_eq!(format_snap_id("snap-short"), "snap-short");
    }
}
