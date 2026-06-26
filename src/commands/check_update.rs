// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zacxiom check-update` — GitHub release version check command.

use std::process::Command;

const GITHUB_API_URL: &str = "https://api.github.com/repos/oxyzenQ/zacxiom/releases/latest";
const RELEASES_URL: &str = "https://github.com/oxyzenQ/zacxiom/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, PartialEq, Eq)]
enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
    CurrentIsNewer,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

impl SemVer {
    fn parse(version: &str) -> Option<Self> {
        let version = version.trim();
        let version = version.strip_prefix('v').unwrap_or(version);
        let version = version
            .split_once('-')
            .map_or(version, |(stable, _)| stable);
        let mut parts = version.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

fn normalize_version(version: &str) -> String {
    let version = version.trim();
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

fn compare_versions(current: &str, latest: &str) -> UpdateStatus {
    match (SemVer::parse(current), SemVer::parse(latest)) {
        (Some(current), Some(latest)) if current == latest => UpdateStatus::UpToDate,
        (Some(current), Some(latest)) if current > latest => UpdateStatus::CurrentIsNewer,
        _ => UpdateStatus::UpdateAvailable,
    }
}

fn extract_tag_name(json: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let rest = json.get(json.find(key)? + key.len()..)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn interpret_curl_exit(code: i32) -> &'static str {
    match code {
        6 => "DNS resolution failed",
        7 => "connection refused",
        28 => "network request timed out",
        35 => "SSL/TLS handshake failed",
        _ => "network request failed",
    }
}

fn interpret_http_status(code: u16) -> &'static str {
    match code {
        403 => "GitHub API request was rate-limited or forbidden",
        404 => "no latest GitHub release found for oxyzenQ/zacxiom",
        _ => "GitHub API returned an unexpected error",
    }
}

pub fn check_update() {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--max-time",
            "15",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: zacxiom-check-update",
            "--write-out",
            "\n%{http_code}",
            GITHUB_API_URL,
        ])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("zacxiom update check failed: curl is not available on PATH");
            } else {
                eprintln!("zacxiom update check failed: {e}");
            }
            return;
        }
    };

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        eprintln!("zacxiom update check failed: {}", interpret_curl_exit(code));
        return;
    }

    let raw = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("zacxiom update check failed: response was not valid UTF-8");
            return;
        }
    };

    let (body, status_str) = match raw.rsplit_once('\n') {
        Some(pair) => pair,
        None => {
            eprintln!("zacxiom update check failed: GitHub API response was malformed");
            return;
        }
    };
    let status: u16 = status_str.trim().parse().unwrap_or(0);
    if status != 200 {
        eprintln!(
            "zacxiom update check failed: {}",
            interpret_http_status(status)
        );
        return;
    }

    let latest_tag = match extract_tag_name(body) {
        Some(t) => t,
        None => {
            eprintln!("zacxiom update check failed: could not parse latest release tag from GitHub response");
            return;
        }
    };

    let commit_hash = option_env!("ZACXIOM_GIT_HASH").unwrap_or("unknown");

    match compare_versions(CURRENT_VERSION, &latest_tag) {
        UpdateStatus::CurrentIsNewer => {
            println!("zacxiom update check");
            println!();
            println!("  Local build is newer than the latest published release.");
            println!();
            println!("  Current build:");
            println!(
                "    {} (commit {})",
                normalize_version(CURRENT_VERSION),
                commit_hash
            );
            println!();
            println!("  Latest release:");
            println!("    {}", normalize_version(&latest_tag));
            println!();
            println!("  Source:  {RELEASES_URL}");
        }
        UpdateStatus::UpToDate => {
            println!("zacxiom update check");
            println!("Current: {}", normalize_version(CURRENT_VERSION));
            println!("Latest:  {}", normalize_version(&latest_tag));
            println!("Status:  up to date");
            println!("Source:  {RELEASES_URL}");
        }
        UpdateStatus::UpdateAvailable => {
            println!("zacxiom update check");
            println!("Current: {}", normalize_version(CURRENT_VERSION));
            println!("Latest:  {}", normalize_version(&latest_tag));
            println!("Status:  update available");
            println!("Source:  {RELEASES_URL}");
        }
    }
}
