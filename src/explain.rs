// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Explainability engine v2 — domain-centric storage advisor.
//!
//! v6.2.2: When user runs `zacxiom explain ~/.rustup`, they get
//! a Rustup explanation — not a file-by-file dump.
//! Think like a storage advisor, not a file inspector.

use crate::confidence::{confidence, Tier};
use crate::rules::ClassifiedFile;
use crate::simulator;

/// A full explainability card for a domain.
pub struct Explanation {
    pub title: String,
    pub what: String,
    pub size: String,
    pub tier: Tier,
    pub why_safe: String,
    pub consequence: String,
    pub recommendation: String,
    pub file_count: Option<usize>,
}

/// Determine what domain a path belongs to, then explain it.
pub fn explain_path(path: &str, classified: &[ClassifiedFile]) -> Explanation {
    let tier = if classified.is_empty() {
        Tier::Maximum
    } else {
        classified
            .iter()
            .map(confidence)
            .max()
            .unwrap_or(Tier::Maximum)
    };

    let total_size: u64 = classified.iter().map(|f| f.size).sum();
    let domain_name = infer_domain_name(path, classified);

    explain_domain(&domain_name, total_size, tier, classified.len())
}

/// Infer a human-readable domain name from a path + classified files.
fn infer_domain_name(path: &str, classified: &[ClassifiedFile]) -> String {
    // Use domain classification from the first classified file
    if let Some(f) = classified.first() {
        let d = format!("{:?}", f.cache_domain);
        if d != "Unknown" {
            return d;
        }
    }

    // Protected/special paths — explain accurately, never as "cached"
    let lower = path.to_lowercase();
    if lower.contains(".config") && !lower.contains(".config/") {
        // ~/.config itself — not a cache
        return "Configuration Directory".into();
    }
    if lower.contains(".ssh") {
        return "SSH Keys & Credentials".into();
    }
    if lower.contains(".gnupg") || lower.contains(".gpg") {
        return "GPG Keys".into();
    }
    if lower.contains(".local/share") && !lower.contains(".local/share/") {
        return "User Application Data".into();
    }
    if lower.contains(".password") || lower.contains("keyring") {
        return "Credentials & Secrets".into();
    }
    if lower.contains(".mozilla") && lower.contains("profile") {
        return "Firefox Profile".into();
    }
    if lower.contains(".chrome") && lower.contains("default") {
        return "Chrome Profile".into();
    }
    if lower.contains("wallet") || lower.contains("kde") {
        return "Desktop Wallet".into();
    }
    if lower.contains("systemd") || lower.contains("/etc/") {
        return "System Configuration".into();
    }

    // Fallback: path-based heuristics
    let lower = path.to_lowercase();
    if lower.contains(".cargo") || lower.contains("cargo") {
        return "Cargo Registry".into();
    }
    if lower.contains("rustup") {
        return "Rustup Toolchains".into();
    }
    if lower.contains(".npm") || lower.contains("npm") {
        return "npm Package Cache".into();
    }
    if lower.contains("pip") || lower.contains(".cache/pip") {
        return "pip Package Cache".into();
    }
    if lower.contains(".cache/uv") {
        return "uv Cache".into();
    }
    if lower.contains("docker") || lower.contains("containerd") {
        return "Docker Storage".into();
    }
    if lower.contains("steam") {
        return "Steam Game Cache".into();
    }
    if lower.contains("proton") || lower.contains("compatdata") {
        return "Proton Compatibility Data".into();
    }
    if lower.contains("firefox") || lower.contains("mozilla") {
        return "Firefox Browser Cache".into();
    }
    if lower.contains("chrome") || lower.contains("chromium") {
        return "Chrome Browser Cache".into();
    }
    if lower.contains("brave") {
        return "Brave Browser Cache".into();
    }
    if lower.contains("discord") {
        return "Discord App Cache".into();
    }
    if lower.contains("vscode") {
        return "VS Code Cache".into();
    }
    if lower.contains("huggingface") || lower.contains("hf.co") {
        return "HuggingFace Model Cache".into();
    }
    if lower.contains("ollama") {
        return "Ollama Model Storage".into();
    }
    if lower.contains("gradle") {
        return "Gradle Build Cache".into();
    }
    if lower.contains(".m2") || lower.contains("maven") {
        return "Maven Repository".into();
    }
    if lower.contains("node_modules") {
        return "Node.js Dependencies".into();
    }
    if lower.contains("trash") {
        return "Desktop Trash".into();
    }
    if lower.contains("downloads") {
        return "Downloads Directory".into();
    }

    // Extract last meaningful path component
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(|s| {
            if s.is_empty() || s == "." || s == ".." {
                path.to_string()
            } else {
                s.to_string()
            }
        })
        .unwrap_or_else(|| path.to_string())
}

/// Generate a domain-level explanation.
pub fn explain_domain(domain: &str, total_size: u64, tier: Tier, file_count: usize) -> Explanation {
    let lower = domain.to_lowercase();
    let (what, why, consequence) = if lower.contains("ssh")
        || lower.contains("key")
        || lower.contains("credential")
    {
        (
            "SSH keys, authorized_keys, and credential files.",
            "These are your identity and access credentials. Never auto-clean. Manual review only.",
            "Deleting SSH keys permanently removes access to remote systems. Cannot be regenerated.",
        )
    } else if lower.contains("gpg") {
        (
            "GPG/OpenPGP encryption keys and keyring data.",
            "These are cryptographic identities. Lost keys cannot be recovered. Manual review only.",
            "Deleting keys means losing the ability to decrypt data encrypted to them.",
        )
    } else if lower.contains("config") && !lower.contains("cache") {
        (
            "Application configuration files and user preferences.",
            "These contain your customized settings. Most apps recreate defaults if deleted, but customizations are lost.",
            "Apps reset to factory defaults. Custom settings, shortcuts, and preferences are lost.",
        )
    } else if lower.contains("profile")
        && (lower.contains("firefox") || lower.contains("chrome") || lower.contains("browser"))
    {
        (
            "Browser profile data — bookmarks, history, saved passwords, extensions.",
            "Browser profiles contain personal data that cannot be regenerated. Never auto-clean.",
            "Bookmarks, saved passwords, and browsing history would be permanently lost.",
        )
    } else if lower.contains("local/share") || lower.contains("app data") {
        (
            "User application data — saved states, databases, user-generated content.",
            "This is where applications store your actual data. Review file-by-file before deleting.",
            "Application data may be permanently lost. Some apps sync to cloud, others do not.",
        )
    } else if lower.contains("wallet") || lower.contains("password") {
        (
            "Encrypted wallet and password storage.",
            "Contains saved credentials. Never auto-clean. Loss means losing all saved passwords.",
            "All saved passwords and wallet data permanently lost.",
        )
    } else if lower.contains("browser") {
        (
            "Browser cache, temporary internet files, and service worker storage.",
            "Browsers rebuild their cache automatically as you browse. No bookmarks, passwords, or settings are affected.",
            "Websites may load slightly slower on first visit until the cache rebuilds.",
        )
    } else if lower.contains("cargo") {
        (
            "Downloaded Rust crate files used by Cargo for building projects.",
            "Cargo automatically re-downloads missing crates on the next `cargo build`. No code or data lost.",
            "Next `cargo build` may spend 2-5 minutes downloading crates.",
        )
    } else if lower.contains("rustup") || lower.contains("toolchain") {
        (
            "Installed Rust toolchain components downloaded by rustup.",
            "rustup re-downloads toolchains on `rustup update`. Only old/unused versions are targeted.",
            "Version-specific builds may need the toolchain re-downloaded. Active toolchain is safe.",
        )
    } else if lower.contains("docker") || lower.contains("container") {
        (
            "Docker image layers, build cache, and container storage.",
            "Docker rebuilds images from Dockerfiles. Running containers are NOT affected.",
            "Next `docker build` will rebuild layers from cache or Dockerfile.",
        )
    } else if lower.contains("ai")
        || lower.contains("ml")
        || lower.contains("model")
        || lower.contains("huggingface")
        || lower.contains("ollama")
    {
        (
            "Downloaded AI/ML model files (HuggingFace, Ollama, Torch, etc.).",
            "Models can be re-downloaded from their sources. Checkpoints may have training value — review before deleting.",
            "Models will re-download when needed. Training checkpoints are permanently deleted.",
        )
    } else if lower.contains("npm") || lower.contains("yarn") || lower.contains("pnpm") {
        (
            "Cached JavaScript/TypeScript packages downloaded by npm, yarn, or pnpm.",
            "Package managers re-download from the registry on `npm install`. This is a network cache.",
            "Next install may take longer while packages re-download.",
        )
    } else if lower.contains("pip") || lower.contains("python") || lower.contains("uv") {
        (
            "Downloaded Python packages cached by pip or uv.",
            "pip/uv re-downloads packages from PyPI. Virtual environments are separate.",
            "Next `pip install` may take longer. Virtual environments are unaffected.",
        )
    } else if lower.contains("gradle") || lower.contains("maven") {
        (
            "Java/Kotlin build dependencies and build cache.",
            "Gradle/Maven re-download dependencies. Source code is not affected.",
            "Next `gradle build` may take longer to download dependencies.",
        )
    } else if lower.contains("steam") && lower.contains("shader") {
        (
            "Pre-compiled GPU shader cache for Steam games.",
            "Shaders regenerate automatically when the game launches. No game data affected.",
            "Games may have slightly lower FPS for the first few minutes.",
        )
    } else if lower.contains("proton") || lower.contains("compat") {
        (
            "Proton/Wine compatibility data for Steam games (Windows emulation layer).",
            "Steam reinstalls Proton prefixes when launching games. Game saves are typically in Steam Cloud.",
            "Games may take longer to launch first time. Game saves should be unaffected.",
        )
    } else if lower.contains("dxvk") || lower.contains("vkd3d") || lower.contains("mesa") {
        (
            "GPU shader translation cache (DirectX-to-Vulkan/OpenGL).",
            "Automatically regenerated by the GPU driver on next game launch.",
            "Temporary performance dip for 1-5 minutes while shaders recompile.",
        )
    } else if lower.contains("trash") {
        (
            "Files you've already deleted — they're in your Trash directory.",
            "You already chose to delete these files. The Trash is a safety net, not permanent storage.",
            "Files will be permanently removed. Restore from file manager before cleaning if needed.",
        )
    } else if lower.contains("download") {
        (
            "Files in your Downloads directory not accessed recently.",
            "These are files you downloaded. Review before deleting — old ISOs and installers are usually safe.",
            "Files will be permanently deleted. Review the list carefully.",
        )
    } else if lower.contains("node") {
        (
            "Node.js project dependencies (node_modules).",
            "Recreated with `npm install` from package.json. Project code is unaffected.",
            "Next `npm install` will re-download all dependencies.",
        )
    } else if lower.contains("package") || lower.contains("pacman") {
        (
            "Downloaded system packages cached by the package manager.",
            "The package manager re-downloads packages when needed. Installed software is NOT affected.",
            "Future updates may take longer while packages re-download.",
        )
    } else if lower.contains("discord") {
        (
            "Discord application cache and downloaded media.",
            "Discord re-downloads media as needed. Messages and settings are server-side.",
            "Previously viewed images/attachments may re-download.",
        )
    } else if lower.contains("vscode") || lower.contains("visual studio") {
        (
            "VS Code editor cache, extensions cache, and workspace storage.",
            "VS Code re-downloads extensions and rebuilds its cache. Settings are in user config, not cache.",
            "Extensions may need to be re-downloaded. Settings and workspaces are unaffected.",
        )
    } else {
        (
            "Cached or temporary data that can be regenerated.",
            "These files appear to be cache data. No user documents or settings are included.",
            "Applications may need to regenerate this data on next use.",
        )
    };

    Explanation {
        title: domain.to_string(),
        what: what.into(),
        size: simulator::human_size(total_size),
        tier,
        why_safe: why.into(),
        consequence: consequence.into(),
        recommendation: match tier {
            Tier::Maximum => "Safe to reclaim if disk space needed.".into(),
            Tier::High => "Safe with review. Use `zacxiom clean --smart`.".into(),
            Tier::Moderate => {
                "Review recommended. Use `zacxiom clean --force` after review.".into()
            }
            Tier::Low | Tier::Minimal => "Manual review required. Do not auto-clean.".into(),
            Tier::Protected => "Will never be cleaned automatically by Zacxiom.".into(),
        },
        file_count: Some(file_count),
    }
}

/// Explain a single file (fallback when no domain match).
pub fn explain_file(file: &ClassifiedFile) -> Explanation {
    let tier = confidence(file);

    let what = match file.cache_domain {
        crate::rules::CacheDomain::Browser => "Browser cache entry",
        crate::rules::CacheDomain::BuildArtifact => "Build artifact or dependency",
        crate::rules::CacheDomain::PackageManager => "Package manager cache",
        crate::rules::CacheDomain::Developer => "Developer tooling cache",
        crate::rules::CacheDomain::System => "System cache file",
        crate::rules::CacheDomain::UserData => "User cache data",
        crate::rules::CacheDomain::Unknown => "Unclassified file",
    };

    Explanation {
        title: file.path.clone(),
        what: what.into(),
        size: simulator::human_size(file.size),
        tier,
        why_safe: if file.risk_reasons.is_empty() {
            "No specific risk factors detected.".into()
        } else {
            file.risk_reasons.join("; ")
        },
        consequence: match tier {
            Tier::Maximum => "Can be regenerated automatically. No data loss.",
            Tier::High => "May require time to regenerate. No user data affected.",
            Tier::Moderate => "Probably safe but review the risk reasons.",
            Tier::Low => "May contain valuable data. Manual review recommended.",
            Tier::Minimal => "Potentially important. Do not delete without review.",
            Tier::Protected => "System or user-critical. Never deleted.",
        }
        .into(),
        recommendation: match tier {
            Tier::Maximum => "Safe to delete",
            Tier::High => "Safe with --smart",
            Tier::Moderate => "Review before deleting",
            Tier::Low | Tier::Minimal => "Do not auto-delete",
            Tier::Protected => "Will never be deleted",
        }
        .into(),
        file_count: Some(1),
    }
}

/// Render an explanation card — clean, readable, no box-drawing.
pub fn render_card(exp: &Explanation) -> String {
    let mut out = String::new();
    let stars = exp.tier.stars();

    // Title line
    out.push_str(&format!("\n{}  {}\n", stars, exp.title));
    out.push_str(&format!("{}\n", "─".repeat(60)));

    // Body
    out.push_str(&format!("  What:      {}\n", exp.what));
    out.push_str(&format!("  Size:      {}\n", exp.size));
    if let Some(n) = exp.file_count {
        if n > 1 {
            out.push_str(&format!("  Files:     {}\n", n));
        }
    }
    out.push_str(&format!("  Why safe:  {}\n", exp.why_safe));
    out.push_str(&format!("  If deleted: {}\n", exp.consequence));
    out.push_str(&format!("  Recommend: {}\n", exp.recommendation));
    out
}
