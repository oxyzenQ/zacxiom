// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Shared pipeline helpers — context, classification dispatch, utilities.
//!
//! Extracted from main.rs to keep the entrypoint lean.

use crate::rules;
use crate::scanner;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;

pub const BUILD_TARGET: &str = {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "linux-aarch64"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64")
    )))]
    {
        "unknown"
    }
};

pub struct RunContext {
    pub open_files: HashSet<PathBuf>,
    pub history_cleaned: HashSet<String>,
    pub health: crate::profiles::HealthMode,
    pub profile: crate::profiles::Profile,
    pub memory: crate::memory::ContextMemory,
}

impl RunContext {
    pub fn new(profile_arg: &str) -> Self {
        RunContext {
            open_files: crate::procfs::build_open_file_set(),
            history_cleaned: {
                let h = crate::history::History::load();
                h.previously_cleaned_paths().into_iter().collect()
            },
            health: crate::profiles::detect_health(),
            profile: crate::profiles::Profile::from_str(profile_arg),
            memory: crate::memory::ContextMemory::load(),
        }
    }
}

pub fn print_version() {
    let h = option_env!("ZACXIOM_GIT_HASH").unwrap_or("unknown");
    println!("zacxiom -V/--version");
    println!("Version: v{}", env!("CARGO_PKG_VERSION"));
    println!("Build: {} ({})", BUILD_TARGET, h);
    println!("Copyright: (c) 2026 rezky_nightky (oxyzenQ)");
    println!("License: GPL-3.0");
    println!("Source: https://github.com/oxyzenQ/zacxiom");
}

pub fn resolve_roots(paths: Vec<String>) -> Vec<PathBuf> {
    if paths.is_empty() {
        scanner::default_scan_roots()
    } else {
        paths.into_iter().map(PathBuf::from).collect()
    }
}

/// Determine optimal thread count based on workload size.
/// Small: 2 threads. Medium: half of logical CPUs. Large: all CPUs.
pub fn optimal_threads(file_count: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    if file_count < 5_000 {
        2.min(cpus)
    } else if file_count < 50_000 {
        (cpus / 2).max(2)
    } else {
        cpus.max(2)
    }
}

pub fn classify(
    entries: Vec<scanner::ScanEntry>,
    ctx: &RunContext,
    threads: usize,
) -> Vec<rules::ClassifiedFile> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let total = entries.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let ctr = counter.clone();

    // Progress reporter thread for large datasets
    let _reporter = if total > 500 {
        Some(std::thread::spawn(move || {
            loop {
                let done = ctr.load(Ordering::Relaxed);
                if done >= total {
                    break;
                }
                let pct = done * 100 / total;
                let bar = 20;
                let filled = pct * bar / 100;
                let done_str = if done >= 1_000_000 {
                    format!("{:.1}M", done as f64 / 1_000_000.0)
                } else if done >= 1_000 {
                    format!("{:.1}K", done as f64 / 1_000.0)
                } else {
                    format!("{done}")
                };
                let total_str = if total >= 1_000_000 {
                    format!("{:.1}M", total as f64 / 1_000_000.0)
                } else if total >= 1_000 {
                    format!("{:.1}K", total as f64 / 1_000.0)
                } else {
                    format!("{total}")
                };
                print!(
                    "\r\x1b[K  {} [{:5}] {:>7} / {:<7}  [{}{}] {:>3}%",
                    crate::color::purple_spinner('⠋'),
                    "CLASSIFY",
                    done_str,
                    total_str,
                    "█".repeat(filled),
                    "░".repeat(bar.saturating_sub(filled)),
                    pct,
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            print!("\r\x1b[K");
            std::io::stdout().flush().ok();
        }))
    } else {
        None
    };

    let result = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("rayon pool")
        .install(|| {
            entries
                .into_par_iter()
                .map(|e| {
                    let d = crate::cache::classify(&e.path);
                    let o = crate::ownership::detect(&e.path);
                    let path_str = e.path.to_string_lossy().into_owned();
                    let age = crate::risk::file_age_days(&path_str);
                    let modif = ctx.memory.risk_modifier(&path_str);
                    let mut scored = crate::risk::score_v3(&crate::risk::RiskSignals {
                        path: &path_str,
                        size: e.size,
                        domain: &d,
                        ownership: &o,
                        open_files: Some(&ctx.open_files),
                        history_cleaned: Some(&ctx.history_cleaned),
                        memory_modifier: modif,
                        age_days: age,
                    });
                    if modif != 0.0 {
                        scored.risk_reasons.push(format!(
                            "memory: adaptive modifier {modif:+.3} (sessions: {})",
                            ctx.memory.sessions
                        ));
                    }
                    // v6.3.1: bridge — fast classify, zero-heap category
                    let eng = crate::engine::classify_fast(&e.path);
                    scored.engine_category = eng.0.to_string();
                    scored.engine_confidence = eng.1;

                    // v7: Bridge — engine category overrides legacy Decision
                    // to align semantic identity with cleanup policy.
                    // Toolchain, installed software, dependency source, and
                    // downloaded artifacts all require --smart — not auto-cleanable.
                    if scored.decision == rules::Decision::Safe {
                        if eng.0 == "Toolchain Installation"
                            || eng.0 == "Toolchain Manager"
                            || eng.0 == "Installed Software"
                            || eng.0 == "Dependency Source"
                        {
                            scored.decision = rules::Decision::LowRisk;
                            scored.risk_reasons.push(
                                "Not disposable cache — regenerable but expensive to restore, requires --smart".into(),
                            );
                        }
                        // Downloaded artifacts (cargo registry, SDKs) — also need --smart
                        else if eng.0.contains("Downloaded") {
                            scored.decision = rules::Decision::LowRisk;
                            scored.risk_reasons.push(
                                "Downloaded artifact: regenerable but expensive to restore".into(),
                            );
                        }
                    }
                    counter.fetch_add(1, Ordering::Relaxed);
                    scored
                })
                .collect()
        });
    result
}

pub fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let tod = secs % 86400;
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let diy = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366
        } else {
            365
        };
        if d < diy {
            break;
        }
        d -= diy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mdays: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut rem = d;
    let mut mo = 1i64;
    for &md in &mdays {
        if rem < md {
            break;
        }
        rem -= md;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{:02}T{h:02}:{m:02}:{s:02}Z", rem + 1)
}

/// Extract top storage contributors from classified files (v6.2.1).
/// Groups by path prefix patterns to show "where storage is going".
pub fn top_contributors(
    files: &[rules::ClassifiedFile],
    limit: usize,
) -> Vec<(String, usize, u64)> {
    use std::collections::HashMap;

    // Group by path-derived contributor name
    let mut groups: HashMap<String, (usize, u64)> = HashMap::new();

    for f in files {
        let name = contributor_name(&f.path);
        let entry = groups.entry(name).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += f.size;
    }

    let mut sorted: Vec<(String, usize, u64)> =
        groups.into_iter().map(|(k, (c, s))| (k, c, s)).collect();
    sorted.sort_by_key(|(_, _, s)| std::cmp::Reverse(*s));
    sorted.truncate(limit);
    sorted
}

/// Derive a human-readable contributor name from a file path.
pub fn contributor_name(path: &str) -> String {
    let lower = path.to_lowercase();

    // Browser-specific
    if lower.contains("firefox") || lower.contains("mozilla") {
        return "Firefox".into();
    }
    if lower.contains("chromium") {
        return "Chromium".into();
    }
    if lower.contains("chrome") {
        return "Google Chrome".into();
    }
    if lower.contains("brave") {
        return "Brave".into();
    }
    if lower.contains("edge") {
        return "Microsoft Edge".into();
    }

    // Developer tools
    if lower.contains(".cargo") {
        return "Cargo (Rust)".into();
    }
    if lower.contains("rustup") {
        return "Rustup".into();
    }
    if lower.contains(".npm") || lower.contains("npm") {
        return "npm".into();
    }
    if lower.contains("pnpm") {
        return "pnpm".into();
    }
    if lower.contains("yarn") {
        return "Yarn".into();
    }
    if lower.contains("pip") {
        return "pip (Python)".into();
    }
    if lower.contains("/uv/") || lower.contains(".cache/uv") {
        return "uv (Python)".into();
    }
    if lower.contains("docker") || lower.contains("containers") {
        return "Docker".into();
    }
    if lower.contains("gradle") {
        return "Gradle".into();
    }
    if lower.contains("maven") || lower.contains(".m2") {
        return "Maven".into();
    }
    if lower.contains("node_modules") {
        return "Node.js (node_modules)".into();
    }

    // Gaming
    if lower.contains("steam") {
        return "Steam".into();
    }
    if lower.contains("lutris") {
        return "Lutris".into();
    }
    if lower.contains("heroic") {
        return "Heroic".into();
    }
    if lower.contains("compatdata") || lower.contains("proton") {
        return "Proton (Steam)".into();
    }
    if lower.contains("dxvk") || lower.contains("vkd3d") || lower.contains("mesa") {
        return "Shader Cache".into();
    }

    // Desktop apps
    if lower.contains("discord") {
        return "Discord".into();
    }
    if lower.contains("spotify") {
        return "Spotify".into();
    }
    if lower.contains("slack") {
        return "Slack".into();
    }
    if lower.contains("vscode") || lower.contains("visual studio") {
        return "VS Code".into();
    }
    if lower.contains("jetbrains") || lower.contains("intellij") {
        return "JetBrains IDE".into();
    }
    if lower.contains("thunderbird") {
        return "Thunderbird".into();
    }

    // AI/ML
    if lower.contains("huggingface") {
        return "HuggingFace".into();
    }
    if lower.contains("ollama") {
        return "Ollama".into();
    }
    if lower.contains("torch") || lower.contains("pytorch") {
        return "PyTorch".into();
    }

    // System
    if lower.contains("/tmp/") {
        return "Temporary Files".into();
    }
    if lower.contains("trash") {
        return "Desktop Trash".into();
    }
    if lower.contains("downloads") {
        return "Downloads".into();
    }
    if lower.contains("pacman") || lower.contains("yay") || lower.contains("paru") {
        return "Package Manager".into();
    }

    // Fallback: extract app name from path
    path.split('/')
        .find(|p| p.contains(".cache") || p.contains(".config") || p.contains(".local"))
        .map(|s| {
            let parts: Vec<&str> = s.split('/').collect();
            if parts.len() >= 2 {
                parts[parts.len() - 1].to_string()
            } else {
                s.to_string()
            }
        })
        .unwrap_or_else(|| "Other".into())
}
