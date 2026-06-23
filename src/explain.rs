// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Explainability engine v3 — accurate path-aware domain detection.
//!
//! v6.2.4: Fixed file-vs-directory detection, user directory handling,
//! .cache classification, and accurate path resolution.
//! No more "21 GB .zshrc" hallucinations.

use crate::confidence::{confidence, Tier};
use crate::rules::ClassifiedFile;
use crate::simulator;

/// A full explainability card for a domain or file.
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

/// Infer a human-readable domain name from a path.
/// v6.2.4: Handles files, user directories, cache, and special paths.
fn infer_domain_name(path: &str, classified: &[ClassifiedFile]) -> String {
    let lower = path.to_lowercase();

    // ── Regular files: classify by extension/path ──────────────
    if !path.ends_with('/') {
        // It's a file (or at least not explicitly a directory)
        if lower.ends_with(".zshrc")
            || lower.ends_with(".bashrc")
            || lower.ends_with(".profile")
            || lower.ends_with(".bash_profile")
            || lower.ends_with(".zprofile")
            || lower.contains("/.zshrc")
        {
            return "Shell Config File".into();
        }
        if lower.contains(".gitconfig") || lower.contains(".git-credentials") {
            return "Git Config File".into();
        }
        if lower.ends_with(".toml")
            || lower.ends_with(".yaml")
            || lower.ends_with(".json")
            || lower.ends_with(".ini")
            || lower.ends_with(".conf")
        {
            return "Configuration File".into();
        }
        if lower.ends_with(".env") || lower.contains(".env.") {
            return "Environment Config File".into();
        }
        if lower.ends_with(".pub") || lower.ends_with(".pem") || lower.ends_with(".key") {
            return "Key File".into();
        }
        if lower.contains(".ssh/") {
            return "SSH Key/Config File".into();
        }
    }

    // ── User directories (never cache, never auto-clean) ───────
    if lower.contains("/desktop") && !lower.contains("/desktop/") {
        return "User Desktop".into();
    }
    if lower.contains("/documents") && !lower.contains("/documents/") {
        return "User Documents".into();
    }
    if lower.contains("/music") && !lower.contains("/music/") {
        return "User Music".into();
    }
    if lower.contains("/pictures") && !lower.contains("/pictures/") {
        return "User Pictures".into();
    }
    if lower.contains("/videos") && !lower.contains("/videos/") {
        return "User Videos".into();
    }
    if lower.contains("/downloads") && !lower.contains("/downloads/") {
        return "Downloads Directory".into();
    }
    if lower.contains("/public") && !lower.contains("/public/") {
        return "Public Directory".into();
    }
    if lower.contains("/templates") && !lower.contains("/templates/") {
        return "Templates Directory".into();
    }

    // ── Cache directory (explicit — IS cache, NOT user data) ──
    if lower.ends_with("/.cache") || lower.contains("/.cache/") {
        return "User Cache".into();
    }

    // ── Configuration directories ──────────────────────────────
    if lower.ends_with("/.config") {
        return "Configuration Directory".into();
    }
    if lower.contains("/.config/") && !lower.contains("/cache") {
        return "Application Configuration".into();
    }

    // ── Protected paths ────────────────────────────────────────
    if lower.contains(".ssh") {
        return "SSH Keys & Credentials".into();
    }
    if lower.contains(".gnupg") || lower.contains(".gpg") {
        return "GPG Keys".into();
    }
    if lower.contains(".local/share") {
        return "User Application Data".into();
    }
    if lower.contains("wallet") || lower.contains("password") || lower.contains("keyring") {
        return "Credentials & Secrets".into();
    }
    if lower.contains("mozilla") && lower.contains("profile") {
        return "Firefox Profile".into();
    }

    // ── Use classified file's domain if available ──────────────
    if let Some(f) = classified.first() {
        let d = format!("{:?}", f.cache_domain);
        if d != "Unknown" {
            return d;
        }
    }

    // ── Tooling-specific paths ─────────────────────────────────
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

    // ── Fallback ───────────────────────────────────────────────
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
    let (what, why, consequence, recommendation) = match_domain_explanation(&lower, domain, &tier);

    Explanation {
        title: domain.to_string(),
        what: what.to_string(),
        size: simulator::human_size(total_size),
        tier,
        why_safe: why.to_string(),
        consequence: consequence.to_string(),
        recommendation: match recommendation {
            Some(r) => r.to_string(),
            None => match tier {
                Tier::Maximum => "Safe to reclaim if disk space needed.".into(),
                Tier::High => "Safe with review. Use `zacxiom clean --smart`.".into(),
                Tier::Moderate => {
                    "Review recommended. Use `zacxiom clean --force` after review.".into()
                }
                Tier::Low | Tier::Minimal => "Manual review required. Do not auto-clean.".into(),
                Tier::Protected => "Will never be cleaned automatically by Zacxiom.".into(),
            },
        },
        file_count: Some(file_count),
    }
}

fn match_domain_explanation(
    lower: &str,
    _domain: &str,
    _tier: &Tier,
) -> (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
) {
    // ── Protected / User data — accurate warnings ──────────────
    if lower.contains("shell config") || lower.contains(".zshrc") || lower.contains(".bashrc") {
        return (
            "Shell configuration file — defines your terminal environment, aliases, and PATH settings.",
            "This is a configuration file, not cache. It contains your personal shell customizations. Manual review only.",
            "Deleting this file resets your shell environment to defaults. Custom aliases, PATH, and prompt settings are lost.",
            Some("Do not auto-delete. Review manually before removing."),
        );
    }
    if lower.contains("config file") && !lower.contains("application") {
        return (
            "Configuration file — application or system settings.",
            "Contains customized settings. Not regenerable. Manual review required.",
            "Application or system settings reset to defaults.",
            Some("Do not auto-delete. Manual review only."),
        );
    }
    if lower.contains("key file")
        || lower.contains("credential")
        || lower.contains("ssh")
        || lower.contains("gpg")
    {
        return (
            "Security credential, key, or identity file.",
            "These are cryptographic identities and access credentials. Never auto-clean. Loss means permanent loss of access.",
            "Deleting keys permanently removes access to systems or encrypted data. Cannot be regenerated.",
            Some("Never delete without understanding consequences."),
        );
    }
    if lower.contains("user desktop")
        || lower.contains("documents")
        || lower.contains("music")
        || lower.contains("pictures")
        || lower.contains("videos")
        || lower.contains("templates")
        || lower.contains("public dir")
    {
        return (
            "Personal files and user content — documents, media, or desktop files.",
            "These are your personal files. Zacxiom does NOT auto-clean user content. Review before deleting anything here.",
            "Personal files would be permanently deleted. Not recoverable from cache or cloud.",
            Some("Never auto-cleaned. Review each file before deleting."),
        );
    }
    if lower.contains("downloads") {
        return (
            "Files in your Downloads directory.",
            "These are files you downloaded. Some may be important, others are old ISOs/installers. Review before deleting.",
            "Files will be permanently deleted. Review the list carefully.",
            Some("Manual review required. Old installers are usually safe."),
        );
    }
    if lower.contains("firefox profile") || lower.contains("chrome profile") {
        return (
            "Browser profile — bookmarks, history, saved passwords, extensions.",
            "Contains personal browsing data that cannot be regenerated. Never auto-clean.",
            "Bookmarks, saved passwords, and browsing history permanently lost.",
            Some("Never auto-clean browser profiles."),
        );
    }
    if lower.contains("config") && !lower.contains("cache") && !lower.contains("application") {
        return (
            "Application configuration files and user preferences.",
            "Contains your customized settings. Most apps recreate defaults if deleted, but customizations are lost.",
            "Apps reset to factory defaults. Custom settings and preferences are lost.",
            Some("Review before deleting. Settings will be lost."),
        );
    }

    // ── Caches — accurate safety ───────────────────────────────
    if lower.contains("user cache") || (lower.contains("cache") && !lower.contains("config")) {
        return (
            "User application cache data — temporary files stored by desktop and CLI applications.",
            "Applications rebuild their cache automatically. This is designed to be safe to remove. Zacxiom's primary target.",
            "Applications may take slightly longer to start or reload content until caches rebuild.",
            None, // Use default tier-based recommendation
        );
    }
    if lower.contains("browser") && lower.contains("cache") {
        return (
            "Browser cache, temporary internet files, and service worker storage.",
            "Browsers rebuild their cache automatically as you browse. No bookmarks, passwords, or settings are affected.",
            "Websites may load slightly slower on first visit until the cache rebuilds.",
            None,
        );
    }
    if lower.contains("cargo") && !lower.contains("config") {
        return (
            "Downloaded Rust crate files used by Cargo for building projects.",
            "Cargo automatically re-downloads missing crates on the next `cargo build`. No code or data lost.",
            "Next `cargo build` may spend 2-5 minutes downloading crates.",
            None,
        );
    }
    if lower.contains("rustup") || lower.contains("toolchain") {
        return (
            "Installed Rust toolchain components downloaded by rustup.",
            "rustup re-downloads toolchains on `rustup update`. Only old/unused versions are targeted.",
            "Version-specific builds may need the toolchain re-downloaded. Active toolchain is safe.",
            None,
        );
    }
    if lower.contains("docker") || lower.contains("container") {
        return (
            "Docker image layers, build cache, and container storage.",
            "Docker rebuilds images from Dockerfiles. Running containers are NOT affected.",
            "Next `docker build` will rebuild layers from cache or Dockerfile.",
            None,
        );
    }
    if lower.contains("ai")
        || lower.contains("ml")
        || lower.contains("model")
        || lower.contains("huggingface")
        || lower.contains("ollama")
    {
        return (
            "Downloaded AI/ML model files (HuggingFace, Ollama, Torch, etc.).",
            "Models can be re-downloaded from their sources. Checkpoints may have training value — review before deleting.",
            "Models will re-download when needed. Training checkpoints are permanently deleted.",
            Some("Review checkpoints carefully. Models can be re-downloaded."),
        );
    }
    if lower.contains("npm") || lower.contains("yarn") || lower.contains("pnpm") {
        return (
            "Cached JavaScript/TypeScript packages.",
            "Package managers re-download from the registry on `npm install`. This is a network cache.",
            "Next install may take longer while packages re-download.",
            None,
        );
    }
    if lower.contains("pip") || lower.contains("python") || lower.contains("uv") {
        return (
            "Downloaded Python packages cached by pip or uv.",
            "pip/uv re-downloads packages from PyPI. Virtual environments are separate.",
            "Next `pip install` may take longer. Virtual environments are unaffected.",
            None,
        );
    }
    if lower.contains("gradle") || lower.contains("maven") {
        return (
            "Java/Kotlin build dependencies and build cache.",
            "Gradle/Maven re-download dependencies. Source code is not affected.",
            "Next build may take longer to download dependencies.",
            None,
        );
    }
    if lower.contains("steam") && lower.contains("shader") {
        return (
            "Pre-compiled GPU shader cache for Steam games.",
            "Shaders regenerate automatically when the game launches. No game data affected.",
            "Games may have slightly lower FPS for the first few minutes.",
            None,
        );
    }
    if lower.contains("proton") || lower.contains("compat") {
        return (
            "Proton/Wine compatibility data for Steam games (Windows emulation layer).",
            "Steam reinstalls Proton prefixes when launching games. Game saves are typically in Steam Cloud.",
            "Games may take longer to launch first time. Game saves should be unaffected.",
            None,
        );
    }
    if lower.contains("dxvk") || lower.contains("vkd3d") || lower.contains("mesa") {
        return (
            "GPU shader translation cache (DirectX-to-Vulkan/OpenGL).",
            "Automatically regenerated by the GPU driver on next game launch.",
            "Temporary performance dip for 1-5 minutes while shaders recompile.",
            None,
        );
    }
    if lower.contains("trash") {
        return (
            "Files you've already deleted — they're in your Trash directory.",
            "You already chose to delete these files. The Trash is a safety net, not permanent storage.",
            "Files will be permanently removed. Restore from file manager before cleaning if needed.",
            None,
        );
    }
    if lower.contains("node") && lower.contains("modules") {
        return (
            "Node.js project dependencies (node_modules).",
            "Recreated with `npm install` from package.json. Project code is unaffected.",
            "Next `npm install` will re-download all dependencies.",
            None,
        );
    }
    if lower.contains("package") || lower.contains("pacman") {
        return (
            "Downloaded system packages cached by the package manager.",
            "The package manager re-downloads packages when needed. Installed software is NOT affected.",
            "Future updates may take longer while packages re-download.",
            None,
        );
    }
    if lower.contains("discord") {
        return (
            "Discord application cache and downloaded media.",
            "Discord re-downloads media as needed. Messages and settings are server-side.",
            "Previously viewed images/attachments may re-download.",
            None,
        );
    }
    if lower.contains("vscode") || lower.contains("visual studio") {
        return (
            "VS Code editor cache, extensions cache, and workspace storage.",
            "VS Code re-downloads extensions and rebuilds its cache. Settings are in user config, not cache.",
            "Extensions may need to be re-downloaded. Settings and workspaces are unaffected.",
            None,
        );
    }

    // ── Safe fallback ──────────────────────────────────────────
    (
        "Storage that may be safe to clean after review.",
        "No strong risk signals detected, but verify before deleting.",
        "Verify the specific files before proceeding.",
        Some("Review manually before cleaning."),
    )
}

/// Explain a single file (fallback when no domain match).
pub fn explain_file(file: &ClassifiedFile) -> Explanation {
    let tier = confidence(file);
    let domain = infer_domain_name(&file.path, std::slice::from_ref(file));
    explain_domain(&domain, file.size, tier, 1)
}

/// Render an explanation card — clean, readable, no box-drawing.
pub fn render_card(exp: &Explanation) -> String {
    let mut out = String::new();
    let stars = exp.tier.stars();

    // Title line
    out.push_str(&format!("\n{}  {}\n", stars, exp.title));
    out.push_str(&format!("{}\n", "─".repeat(60)));

    // Body
    out.push_str(&format!("  What:       {}\n", exp.what));
    out.push_str(&format!("  Size:       {}\n", exp.size));
    if let Some(n) = exp.file_count {
        if n > 1 {
            out.push_str(&format!("  Files:      {}\n", n));
        }
    }
    out.push_str(&format!("  Safe:       {}\n", exp.why_safe));
    out.push_str(&format!("  If deleted: {}\n", exp.consequence));
    out.push_str(&format!("  Action:     {}\n", exp.recommendation));
    out
}
