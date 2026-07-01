// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom dedup` — find duplicate files by content hash (v14.2).
//!
//! Two-phase: size grouping → SHA-256 for candidates.
//! Read-only — never deletes.

use crate::exclude::ExcludeFilter;
use crate::scanner;
use crate::simulator;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn run_dedup(paths: Vec<String>, min_size: u64, json: bool) {
    let mut all_paths = paths;
    let roots = if all_paths.is_empty() {
        scanner::default_scan_roots()
    } else {
        all_paths.drain(..).map(std::path::PathBuf::from).collect()
    };

    let exclude = ExcludeFilter::empty();
    let entries = scanner::scan(&roots, 0, min_size, true, &exclude);

    // Phase 1: group by size
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();
    for entry in &entries {
        by_size
            .entry(entry.size)
            .or_default()
            .push(entry.path.to_string_lossy().into_owned());
    }

    // Phase 2: for groups with >1 file, compute SHA-256
    let mut dup_groups: Vec<(String, u64, Vec<String>)> = Vec::new(); // (hash, size, paths)
    let mut total_wasted: u64 = 0;

    for (size, files) in &by_size {
        if files.len() < 2 {
            continue;
        }
        let mut by_hash: HashMap<String, Vec<String>> = HashMap::new();
        for path in files {
            if let Ok(hash) = compute_sha256_file(Path::new(path)) {
                by_hash.entry(hash).or_default().push(path.clone());
            }
        }
        for (hash, group) in by_hash {
            if group.len() > 1 {
                total_wasted += size * (group.len() as u64 - 1);
                dup_groups.push((hash, *size, group));
            }
        }
    }

    // Sort by wasted space (size * (count-1)) descending
    dup_groups.sort_by_key(|(_, size, g)| std::cmp::Reverse(*size * (g.len() as u64 - 1)));

    if json {
        let out = serde_json::json!({
            "duplicate_groups": dup_groups.len(),
            "total_wasted_bytes": total_wasted,
            "groups": dup_groups.iter().map(|(hash, size, paths)| {
                serde_json::json!({
                    "hash": hash,
                    "size": size,
                    "count": paths.len(),
                    "paths": paths,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }

    println!("\n━━━ DUPLICATE FILES ━━━");
    println!("  Groups found:    {}", dup_groups.len());
    println!("  Wasted space:    {}", simulator::human_size(total_wasted));
    println!();

    for (hash, size, paths) in dup_groups.iter().take(20) {
        let wasted = size * (paths.len() as u64 - 1);
        println!(
            "  {} × {} (wasted: {})",
            paths.len(),
            simulator::human_size(*size),
            simulator::human_size(wasted)
        );
        for p in paths.iter().take(5) {
            println!("    {p}");
        }
        if paths.len() > 5 {
            println!("    ... and {} more", paths.len() - 5);
        }
        println!("    hash: {hash}");
        println!();
    }

    if dup_groups.len() > 20 {
        println!("  ... and {} more groups", dup_groups.len() - 20);
    }
}

fn compute_sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    use std::io::Read;
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
