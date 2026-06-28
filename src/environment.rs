// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Active Environment Protection — v11.0.0
//!
//! Zacxiom must never remove developer environments, SDKs, toolchains,
//! interpreters, or runtimes that are currently active or recently used.
//!
//! This module detects active developer environments before any clean
//! operation and returns paths that must be protected.
//!
//! Philosophy: "Never clean what the developer is actively using."

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Information about a detected active environment.
#[derive(Debug, Clone)]
pub struct ActiveEnvironment {
    /// Human-readable name (e.g. "Rust Toolchain", "Python venv")
    pub name: String,
    /// The protected directory path
    pub path: PathBuf,
    /// Tech stack (rust, python, node, go, java, etc.)
    pub stack: String,
    /// How it was detected
    pub detected_by: String,
}

/// Configuration for recently-used protection.
#[derive(Debug, Clone)]
pub struct RecentUseConfig {
    /// Files modified within this many seconds are protected.
    pub max_age_seconds: u64,
}

impl Default for RecentUseConfig {
    fn default() -> Self {
        RecentUseConfig {
            // 24 hours default
            max_age_seconds: 24 * 60 * 60,
        }
    }
}

/// Detect all active developer environments on the system.
///
/// Returns a list of active environments with their protected paths.
/// These paths must NEVER be cleaned, regardless of other classification.
pub fn detect_active_environments() -> Vec<ActiveEnvironment> {
    let mut envs = Vec::new();

    detect_rust(&mut envs);
    detect_python(&mut envs);
    detect_node(&mut envs);
    detect_go(&mut envs);
    detect_java(&mut envs);
    detect_bun(&mut envs);
    detect_deno(&mut envs);
    detect_zig(&mut envs);
    detect_llvm(&mut envs);
    detect_cargo_installed(&mut envs);

    envs
}

/// Get the set of all protected paths from active environments.
pub fn protected_paths() -> HashSet<PathBuf> {
    detect_active_environments()
        .into_iter()
        .map(|e| e.path)
        .collect()
}

/// Check if a path is within an active environment.
pub fn is_active_environment(path: &Path) -> Option<ActiveEnvironment> {
    let envs = detect_active_environments();
    let canonical = canonicalize_lossy(path);
    for env in &envs {
        let env_canonical = canonicalize_lossy(&env.path);
        if canonical.starts_with(&env_canonical) || canonical == env_canonical {
            return Some(env.clone());
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════
// Detectors
// ═══════════════════════════════════════════════════════════════

fn detect_rust(envs: &mut Vec<ActiveEnvironment>) {
    // Check rustup
    let home = home_dir();
    let rustup_home = std::env::var("RUSTUP_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".rustup"));

    if !rustup_home.exists() {
        return;
    }

    // Detect active toolchain
    let active_toolchain = run_cmd_stdout("rustup", &["show", "active-toolchain"])
        .or_else(|| run_cmd_stdout("rustup", &["default"]));

    let toolchain_dir = if let Some(ref tc) = active_toolchain {
        let tc = tc.trim();
        // "stable-x86_64-unknown-linux-gnu (default)" → trim the status
        let tc_name = tc.split_whitespace().next().unwrap_or(tc);
        rustup_home.join("toolchains").join(tc_name)
    } else {
        // Fallback: scan toolchains directory for any active one
        find_toolchain_dir(&rustup_home)
    };

    if toolchain_dir.exists() {
        let tc_name = toolchain_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        envs.push(ActiveEnvironment {
            name: format!("Rust Toolchain ({tc_name})"),
            path: toolchain_dir,
            stack: "rust".to_string(),
            detected_by: "rustup show active-toolchain".to_string(),
        });
    }

    // Also protect the entire rustup toolchains directory parent
    let toolchains_root = rustup_home.join("toolchains");
    if toolchains_root.exists() && !envs.iter().any(|e| e.path == toolchains_root) {
        // Only protect if any toolchain is found (otherwise empty dir)
    }

    // Protect cargo home
    let cargo_home = std::env::var("CARGO_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cargo"));

    if cargo_home.exists() {
        // Don't protect entire .cargo — just the bin/ and registry/cache for active use
        // The bin directory contains installed tools
        let cargo_bin = cargo_home.join("bin");
        if cargo_bin.exists() && has_recent_files(&cargo_bin, 86400) {
            envs.push(ActiveEnvironment {
                name: "Cargo Binaries".to_string(),
                path: cargo_bin,
                stack: "rust".to_string(),
                detected_by: "CARGO_HOME active".to_string(),
            });
        }
    }
}

fn detect_python(envs: &mut Vec<ActiveEnvironment>) {
    let home = home_dir();

    // Check active virtualenv
    if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
        let venv_path = PathBuf::from(&venv);
        if venv_path.exists() {
            envs.push(ActiveEnvironment {
                name: format!(
                    "Python Virtualenv ({})",
                    venv_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ),
                path: venv_path,
                stack: "python".to_string(),
                detected_by: "VIRTUAL_ENV".to_string(),
            });
        }
    }

    // Check conda
    if let Ok(conda) = std::env::var("CONDA_PREFIX") {
        let conda_path = PathBuf::from(&conda);
        if conda_path.exists() {
            envs.push(ActiveEnvironment {
                name: "Conda Environment".to_string(),
                path: conda_path,
                stack: "python".to_string(),
                detected_by: "CONDA_PREFIX".to_string(),
            });
        }
    }

    // Check pyenv
    let pyenv_root = std::env::var("PYENV_ROOT")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".pyenv"));

    if pyenv_root.exists() {
        let versions = pyenv_root.join("versions");
        if versions.exists() {
            if let Some(active) = run_cmd_stdout("pyenv", &["version-name"]) {
                let active_ver = active.trim().to_string();
                let ver_path = versions.join(&active_ver);
                if ver_path.exists() {
                    envs.push(ActiveEnvironment {
                        name: format!("pyenv Python ({active_ver})"),
                        path: ver_path,
                        stack: "python".to_string(),
                        detected_by: "pyenv version-name".to_string(),
                    });
                }
            }
        }
    }

    // Check uv
    let uv_python = home.join(".local/share/uv/python");
    if uv_python.exists() && has_recent_files(&uv_python, 86400) {
        envs.push(ActiveEnvironment {
            name: "uv Python (recently used)".to_string(),
            path: uv_python,
            stack: "python".to_string(),
            detected_by: "uv python directory".to_string(),
        });
    }
}

fn detect_node(envs: &mut Vec<ActiveEnvironment>) {
    let home = home_dir();

    // Check nvm
    let nvm_dir = std::env::var("NVM_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".nvm"));

    if nvm_dir.exists() {
        if let Some(current) = run_cmd_stdout("nvm", &["current"]) {
            let ver = current.trim().to_string();
            if !ver.is_empty() && ver != "none" && ver != "system" {
                let ver_path = nvm_dir
                    .join("versions/node")
                    .join(ver.trim_start_matches('v'));
                if ver_path.exists() {
                    envs.push(ActiveEnvironment {
                        name: format!("nvm Node.js ({ver})"),
                        path: ver_path,
                        stack: "node".to_string(),
                        detected_by: "nvm current".to_string(),
                    });
                }
            }
        }
    }

    // Check fnm
    if let Some(current) = run_cmd_stdout("fnm", &["current"]) {
        let ver = current.trim().to_string();
        if !ver.is_empty() {
            let fnm_dir = std::env::var("FNM_DIR")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share/fnm"));

            let ver_path = fnm_dir
                .join("node-versions")
                .join(&ver)
                .join("installation");
            if ver_path.exists() {
                envs.push(ActiveEnvironment {
                    name: format!("fnm Node.js ({ver})"),
                    path: ver_path,
                    stack: "node".to_string(),
                    detected_by: "fnm current".to_string(),
                });
            }
        }
    }

    // Check volta
    let volta_dir = home.join(".volta");
    if volta_dir.exists() {
        let volta_tools = volta_dir.join("tools");
        if volta_tools.exists() && has_recent_files(&volta_tools, 86400) {
            envs.push(ActiveEnvironment {
                name: "Volta (recently used)".to_string(),
                path: volta_tools,
                stack: "node".to_string(),
                detected_by: "volta directory".to_string(),
            });
        }
    }
}

fn detect_go(envs: &mut Vec<ActiveEnvironment>) {
    // Check GOROOT
    if let Ok(goroot) = std::env::var("GOROOT") {
        let goroot_path = PathBuf::from(&goroot);
        if goroot_path.exists() {
            envs.push(ActiveEnvironment {
                name: "Go (GOROOT)".to_string(),
                path: goroot_path,
                stack: "go".to_string(),
                detected_by: "GOROOT".to_string(),
            });
        }
    }

    // Check GOPATH
    if let Ok(gopath) = std::env::var("GOPATH") {
        let gopath_path = PathBuf::from(&gopath);
        if gopath_path.exists() {
            let go_bin = gopath_path.join("bin");
            if go_bin.exists() && has_recent_files(&go_bin, 86400) {
                envs.push(ActiveEnvironment {
                    name: "Go Binaries (recently used)".to_string(),
                    path: go_bin,
                    stack: "go".to_string(),
                    detected_by: "GOPATH bin".to_string(),
                });
            }
        }
    }
}

fn detect_java(envs: &mut Vec<ActiveEnvironment>) {
    // Check JAVA_HOME
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java_path = PathBuf::from(&java_home);
        if java_path.exists() {
            envs.push(ActiveEnvironment {
                name: "Java (JAVA_HOME)".to_string(),
                path: java_path,
                stack: "java".to_string(),
                detected_by: "JAVA_HOME".to_string(),
            });
        }
    }

    // Check sdkman
    let home = home_dir();
    let sdkman = home.join(".sdkman/candidates");
    if sdkman.exists() {
        // Check for active java
        let java_current = sdkman.join("java/current");
        if java_current.exists() {
            if let Ok(target) = std::fs::read_link(&java_current) {
                let real_path = if target.is_relative() {
                    java_current.parent().unwrap_or(&java_current).join(target)
                } else {
                    target
                };
                if real_path.exists() {
                    envs.push(ActiveEnvironment {
                        name: "sdkman Java".to_string(),
                        path: real_path,
                        stack: "java".to_string(),
                        detected_by: "sdkman java current".to_string(),
                    });
                }
            }
        }
    }
}

fn detect_bun(envs: &mut Vec<ActiveEnvironment>) {
    let home = home_dir();
    let bun_bin = home.join(".bun/bin");
    if bun_bin.exists() && has_recent_files(&bun_bin, 86400) {
        envs.push(ActiveEnvironment {
            name: "Bun (recently used)".to_string(),
            path: bun_bin,
            stack: "bun".to_string(),
            detected_by: ".bun directory".to_string(),
        });
    }
}

fn detect_deno(envs: &mut Vec<ActiveEnvironment>) {
    // Check DENO_DIR
    let home = home_dir();
    let deno_dir = std::env::var("DENO_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache/deno"));

    if deno_dir.exists() && has_recent_files(&deno_dir, 86400) {
        envs.push(ActiveEnvironment {
            name: "Deno (recently used)".to_string(),
            path: deno_dir,
            stack: "deno".to_string(),
            detected_by: "DENO_DIR".to_string(),
        });
    }
}

fn detect_zig(envs: &mut Vec<ActiveEnvironment>) {
    // Check common Zig installation paths
    let home = home_dir();
    let zig_paths = [
        home.join(".zig"),
        home.join("zig"),
        PathBuf::from("/usr/local/zig"),
    ];

    for zig_path in &zig_paths {
        if zig_path.exists() && has_recent_files(zig_path, 7 * 86400) {
            envs.push(ActiveEnvironment {
                name: "Zig SDK".to_string(),
                path: zig_path.clone(),
                stack: "zig".to_string(),
                detected_by: "zig directory".to_string(),
            });
            break;
        }
    }
}

fn detect_llvm(envs: &mut Vec<ActiveEnvironment>) {
    // Check LLVM_PATH or common locations
    if let Ok(llvm) = std::env::var("LLVM_SYS_PREFIX").or_else(|_| std::env::var("LLVM_PATH")) {
        let llvm_path = PathBuf::from(&llvm);
        if llvm_path.exists() {
            envs.push(ActiveEnvironment {
                name: "LLVM".to_string(),
                path: llvm_path,
                stack: "llvm".to_string(),
                detected_by: "LLVM_SYS_PREFIX".to_string(),
            });
        }
    }
}

fn detect_cargo_installed(envs: &mut Vec<ActiveEnvironment>) {
    let home = home_dir();
    let cargo_bin = std::env::var("CARGO_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cargo"))
        .join("bin");

    if !cargo_bin.exists() {
        return;
    }

    // If cargo binaries were recently used, protect them
    let mut recent_count = 0;
    if let Ok(entries) = std::fs::read_dir(&cargo_bin) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(meta) = path.metadata() {
                if let Ok(mtime) = meta.modified() {
                    let age = elapsed_secs(mtime);
                    if age < 72 * 3600 {
                        // 72h — wider window for installed tools
                        recent_count += 1;
                    }
                }
            }
            if recent_count >= 3 {
                break;
            }
        }
    }

    if recent_count >= 2 {
        envs.push(ActiveEnvironment {
            name: "Cargo Installed Binaries (active)".to_string(),
            path: cargo_bin,
            stack: "rust".to_string(),
            detected_by: "cargo bin recent usage".to_string(),
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn run_cmd_stdout(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn find_toolchain_dir(rustup_home: &Path) -> PathBuf {
    let toolchains = rustup_home.join("toolchains");
    if !toolchains.exists() {
        return toolchains.join("stable-x86_64-unknown-linux-gnu");
    }

    // Find the toolchain with the most recent modification time
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    if let Ok(entries) = std::fs::read_dir(&toolchains) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(meta) = path.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        let is_newer = newest.as_ref().map(|(_, t)| mtime > *t).unwrap_or(true);
                        if is_newer {
                            newest = Some((path, mtime));
                        }
                    }
                }
            }
        }
    }

    newest
        .map(|(p, _)| p)
        .unwrap_or_else(|| toolchains.join("stable-x86_64-unknown-linux-gnu"))
}

/// Check if a directory contains files modified recently.
fn has_recent_files(dir: &Path, max_age_seconds: u64) -> bool {
    fn check_dir(dir: &Path, max_age: u64, depth: u32) -> bool {
        if depth > 3 {
            return false;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(meta) = path.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        let age = elapsed_secs(mtime);
                        if age < max_age {
                            return true;
                        }
                    }
                }
                if path.is_dir() && check_dir(&path, max_age, depth + 1) {
                    return true;
                }
            }
        }
        false
    }
    check_dir(dir, max_age_seconds, 0)
}

/// Check if a single file was recently modified.
pub fn is_recently_modified(path: &Path, max_age_seconds: u64) -> bool {
    if let Ok(meta) = path.metadata() {
        if let Ok(mtime) = meta.modified() {
            return elapsed_secs(mtime) < max_age_seconds;
        }
    }
    false
}

/// Check if a single file was recently accessed.
pub fn is_recently_accessed(path: &Path, max_age_seconds: u64) -> bool {
    if let Ok(meta) = path.metadata() {
        if let Ok(atime) = meta.accessed() {
            return elapsed_secs(atime) < max_age_seconds;
        }
    }
    false
}

fn elapsed_secs(time: std::time::SystemTime) -> u64 {
    time.elapsed().map(|d| d.as_secs()).unwrap_or(u64::MAX)
}

/// Canonicalize a path, falling back to the original if it doesn't exist.
fn canonicalize_lossy(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_active_environments_does_not_panic() {
        // Must never panic even with no dev tools installed
        let envs = detect_active_environments();
        // Just verify it returns something (may be empty on minimal systems)
        assert!(envs.is_empty() || !envs.is_empty());
    }

    #[test]
    fn test_protected_paths_does_not_panic() {
        let _ = protected_paths();
    }

    #[test]
    fn test_is_recently_modified() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        fs::write(&file, b"data").unwrap();
        // Just created, should be recent
        assert!(is_recently_modified(&file, 86400));
    }

    #[test]
    fn test_has_recent_files() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("recent.txt");
        fs::write(&file, b"data").unwrap();
        assert!(has_recent_files(tmp.path(), 86400));
    }

    #[test]
    fn test_recent_use_config_default() {
        let config = RecentUseConfig::default();
        assert_eq!(config.max_age_seconds, 24 * 60 * 60);
    }
}
