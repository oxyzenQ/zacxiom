// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom viz` — ASCII treemap visualization (v14.2).
//!
//! Shows directory tree sorted by size — read-only, like dust/ncdu.

use crate::exclude::ExcludeFilter;
use crate::scanner;
use crate::simulator;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn run_viz(path: Option<String>, max_depth: usize) {
    let target = path.unwrap_or_else(|| {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".into())
    });

    let roots = vec![PathBuf::from(&target)];
    let exclude = ExcludeFilter::empty();
    let entries = scanner::scan(&roots, max_depth, 1, false, &exclude);

    // Group by parent directory
    let mut dir_sizes: HashMap<String, (u64, usize)> = HashMap::new();
    let mut total_size: u64 = 0;

    for entry in &entries {
        if let Some(parent) = entry.path.parent() {
            let dir_str = parent.to_string_lossy().into_owned();
            let e = dir_sizes.entry(dir_str).or_insert((0, 0));
            e.0 += entry.size;
            e.1 += 1;
            total_size += entry.size;
        }
    }

    // Sort by size descending
    let mut sorted: Vec<_> = dir_sizes.into_iter().collect();
    sorted.sort_by_key(|(_, (size, _))| std::cmp::Reverse(*size));

    println!("\n━━━ DISK USAGE: {target} ━━━");
    println!(
        "  Total: {} across {} files\n",
        simulator::human_size(total_size),
        entries.len()
    );

    let max_bar: usize = 40;
    for (dir, (size, count)) in sorted.iter().take(30) {
        let pct = if total_size > 0 {
            (*size as f64 / total_size as f64 * 100.0) as usize
        } else {
            0
        };
        let filled = if total_size > 0 {
            (*size as f64 / total_size as f64 * max_bar as f64) as usize
        } else {
            0
        };
        let bar = "█".repeat(filled) + &"░".repeat(max_bar.saturating_sub(filled));
        let size_str = simulator::human_size(*size);
        println!("  {size_str:>10} [{bar}] {pct:>3}%  {count:>5} files  {dir}");
    }

    if sorted.len() > 30 {
        println!("  ... and {} more directories", sorted.len() - 30);
    }
    println!();
    println!("  💡 Use 'zacxiom scan <path>' for detailed classification");
    println!("     Use 'zacxiom dedup <path>' to find duplicate files");
}
