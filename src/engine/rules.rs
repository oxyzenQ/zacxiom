// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Structured rule database — replaces giant if/else chains.
//!
//! Rules are ordered by priority. First match wins.
//! Each rule specifies a path pattern, resulting category, and risk level.

use super::types::{Category, RiskLevel};
use std::path::Path;
use std::sync::OnceLock;

/// A single classification rule.
pub struct Rule {
    pub name: &'static str,
    /// Match logic: returns true if this rule applies to the given path.
    pub matches: fn(&Path, &str) -> bool,
    pub category: Category,
    pub risk_level: RiskLevel,
    pub regenerable: bool,
    pub reason: &'static str,
}

/// Build the full rule database in priority order.
/// Cached via OnceLock — called once, shared across all classify() invocations.
/// Priority: system-protected > home-critical > config > cache > app-specific > fallback.
pub fn rule_database() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(build_rules)
}

fn build_rules() -> Vec<Rule> {
    vec![
        // ═══════════════════════════════════════════════════════
        // LAYER 1: System — non-negotiable, never cleanable
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "sys-bin-usr",
            matches: |_, lower| {
                lower.starts_with("/usr/bin/")
                    || lower == "/usr/bin"
                    || lower.starts_with("/usr/local/bin/")
                    || lower == "/usr/local/bin"
            },
            category: Category::SystemBinary,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Installed executable — removing breaks software",
        },
        Rule {
            name: "sys-bin-root",
            matches: |_, lower| {
                lower.starts_with("/bin/")
                    || lower == "/bin"
                    || lower.starts_with("/sbin/")
                    || lower == "/sbin"
                    || lower.starts_with("/usr/sbin/")
                    || lower == "/usr/sbin"
            },
            category: Category::SystemBinary,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "System binary — critical for OS operation",
        },
        Rule {
            name: "sys-etc",
            matches: |_, lower| lower.starts_with("/etc/") || lower == "/etc",
            category: Category::SystemConfiguration,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "System-wide configuration — controls OS behavior",
        },
        Rule {
            name: "sys-lib",
            matches: |_, lower| {
                lower.starts_with("/lib/")
                    || lower == "/lib"
                    || lower.starts_with("/lib64/")
                    || lower == "/lib64"
                    || lower.starts_with("/usr/lib/")
                    || lower.starts_with("/usr/share/")
            },
            category: Category::SystemData,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "System libraries and shared resources",
        },
        Rule {
            name: "sys-boot",
            matches: |_, lower| lower.starts_with("/boot/") || lower == "/boot",
            category: Category::SystemData,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Boot files — deletion prevents system startup",
        },
        Rule {
            name: "sys-virtual",
            matches: |_, lower| {
                lower.starts_with("/dev/")
                    || lower.starts_with("/proc/")
                    || lower.starts_with("/sys/")
            },
            category: Category::VirtualFilesystem,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Virtual kernel interface — not real files",
        },
        Rule {
            name: "sys-opt-bin",
            matches: |_, lower| {
                lower.starts_with("/opt/")
                    && !lower.contains("/cache/")
                    && !lower.contains("/data/")
            },
            category: Category::SystemBinary,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Optional installed software",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 2: User home root — never clean the whole thing
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "home-root",
            matches: |_path, lower| {
                // Exact match for /home/user or /root (not subdirectories)
                let depth = lower.matches('/').count();
                depth <= 2 && (lower.starts_with("/home/") || lower == "/root")
            },
            category: Category::UserHomeRoot,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Home directory — contains mixed personal data and cache",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 3.5: User-installed software — protected
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "user-bin",
            matches: |_, lower| {
                lower.contains("/.local/bin/") && !lower.ends_with("/.local/bin/")
                    || lower.contains("/.cargo/bin/") && !lower.ends_with("/.cargo/bin/")
            },
            category: Category::SystemBinary,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "User-installed binary — removing breaks locally installed software",
        },
        Rule {
            name: "user-bin-dir",
            matches: |_, lower| lower.ends_with("/.local/bin") || lower.ends_with("/.cargo/bin"),
            category: Category::SystemBinary,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "User software directory — contains locally installed executables",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 3.5b: Rustup toolchain manager — all paths protected
        // MUST come before user-downloads (Layer 4) to prevent
        // ~/.rustup/downloads/ from being misclassified as user downloads,
        // and before all cache/config rules to prevent any ~/.rustup/
        // subdirectory from being treated as disposable.
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "rustup-home",
            matches: |_, lower| lower.ends_with("/.rustup") || lower == "/.rustup",
            category: Category::ToolchainManager,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Rust toolchain manager — manages installed Rust compiler versions via rustup",
        },
        Rule {
            name: "rustup-any",
            matches: |_, lower| {
                // Match any path inside ~/.rustup/ (but NOT the directory itself,
                // which is handled by rustup-home above).
                // This catches: toolchains/, downloads/, tmp/, update-hashes/,
                // settings.toml, and any future rustup subdirectories.
                lower.contains("/.rustup/")
            },
            category: Category::ToolchainInstallation,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Rustup toolchain data — installed development tooling, not disposable cache",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 3.6: Version control — never clean
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "vcs-git",
            matches: |_, lower| {
                // Exclude /.cargo/git/ — these are cargo dependency checkouts, not project repos
                !lower.contains("/.cargo/")
                    && (lower.contains("/.git/") || lower.ends_with("/.git"))
            },
            category: Category::SystemData,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Git repository data — deleting corrupts the repository. Not regenerable.",
        },
        Rule {
            name: "vcs-svn-hg",
            matches: |_, lower| {
                lower.contains("/.svn/")
                    || lower.ends_with("/.svn")
                    || lower.contains("/.hg/")
                    || lower.ends_with("/.hg")
            },
            category: Category::SystemData,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Version control metadata — deleting corrupts the working copy",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 4: Security credentials (was L3) — never touch
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "sec-ssh",
            matches: |_, lower| lower.contains(".ssh/") || lower.ends_with("/.ssh"),
            category: Category::SecurityCredential,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "SSH keys and authorized_hosts — identity credentials",
        },
        Rule {
            name: "sec-gpg",
            matches: |_, lower| {
                lower.contains(".gnupg/") || lower.contains(".gpg/") || lower.ends_with("/.gnupg")
            },
            category: Category::SecurityCredential,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "GPG encryption keys — cannot be regenerated",
        },
        Rule {
            name: "sec-key-file",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".pem")
                    || name.ends_with(".key")
                    || name == "id_ed25519"
                    || name == "id_rsa"
                    || name == "id_ecdsa"
                    || name == "authorized_keys"
            },
            category: Category::SecurityCredential,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Cryptographic key file — permanent access loss if deleted",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 4: User content directories — never auto-clean
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "user-desktop",
            matches: |_, lower| {
                lower.contains("/desktop")
                    && (lower.ends_with("/desktop") || lower.contains("/desktop/"))
            },
            category: Category::UserDesktop,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Desktop files — user's primary workspace",
        },
        Rule {
            name: "user-documents",
            matches: |_, lower| {
                lower.contains("/documents")
                    && (lower.ends_with("/documents") || lower.contains("/documents/"))
            },
            category: Category::UserDocument,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Personal documents — may contain irreplaceable files",
        },
        Rule {
            name: "user-music",
            matches: |_, lower| {
                lower.contains("/music") && (lower.ends_with("/music") || lower.contains("/music/"))
            },
            category: Category::UserMedia,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Music/media library — user content",
        },
        Rule {
            name: "user-pictures",
            matches: |_, lower| {
                lower.contains("/pictures")
                    && (lower.ends_with("/pictures") || lower.contains("/pictures/"))
            },
            category: Category::UserMedia,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Picture library — user content",
        },
        Rule {
            name: "user-videos",
            matches: |_, lower| {
                lower.contains("/videos")
                    && (lower.ends_with("/videos") || lower.contains("/videos/"))
            },
            category: Category::UserMedia,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Video library — user content",
        },
        Rule {
            name: "user-downloads",
            matches: |_, lower| {
                // Exclude /.rustup/downloads/ — those are toolchain downloads, not user files
                !lower.contains("/.rustup/")
                    && lower.contains("/downloads")
                    && (lower.ends_with("/downloads") || lower.contains("/downloads/"))
            },
            category: Category::UserDocument,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "Downloaded files — review before deleting; old ISOs/installers usually safe",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 5: Configuration files — manual review
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "config-shell",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                matches!(
                    name,
                    ".zshrc" | ".bashrc" | ".profile" | ".bash_profile" | ".zprofile"
                )
            },
            category: Category::ShellConfiguration,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Shell configuration — defines your terminal environment and aliases",
        },
        Rule {
            name: "config-git",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == ".gitconfig" || name == ".git-credentials"
            },
            category: Category::ApplicationConfiguration,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Git configuration — your identity and settings",
        },
        Rule {
            name: "config-env",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == ".env" || name.starts_with(".env.")
            },
            category: Category::EnvironmentFile,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Environment variables file — may contain secrets and API keys",
        },
        Rule {
            name: "config-app",
            matches: |_, lower| lower.contains("/.config/"),
            category: Category::ApplicationConfiguration,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "Application settings — apps recreate defaults but customizations are lost",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 6: Cache directories — primary targets
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "cache-build-cargo",
            matches: |_, lower| lower.contains("/.cargo/git/") || lower.ends_with("/.cargo/git"),
            category: Category::DownloadedArtifact,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Cargo git checkouts — redownloaded on next cargo build, but large and slow to re-clone",
        },
        Rule {
            name: "cache-build-gradle",
            matches: |_, lower| lower.contains("/.gradle/caches/"),
            category: Category::BuildCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Gradle build cache — redownloaded on next build",
        },
        Rule {
            name: "cache-build-target",
            matches: |_, lower| {
                // Match bare "target", "project/target", and "/target/" subdirectories.
                // Exclude /usr/src (system territory) and /var/ (system data).
                lower == "target"
                    || lower.ends_with("/target")
                    || (lower.contains("/target/")
                        && !lower.starts_with("/usr/")
                        && !lower.starts_with("/var/"))
            },
            category: Category::BuildCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Rust build artifacts — regenerated by cargo build",
        },
        Rule {
            name: "cache-build-node-dist",
            matches: |_, lower| {
                // Node.js build output directories
                lower == "dist"
                    || lower.contains("/dist/")
                    || lower.contains("/.next/")
                    || lower.contains("/.nuxt/")
                    || lower.contains("/.output/")
                    || lower.ends_with("/dist")
                    || lower.ends_with("/.next")
                    || lower.ends_with("/.nuxt")
            },
            category: Category::BuildCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Node.js build output — regenerated by build command",
        },
        Rule {
            name: "cache-build-generic",
            matches: |_, lower| {
                // Generic build output directories used by multiple ecosystems
                // Exclude: system paths, toolchain dirs (/.rustup/, /.cargo/),
                // and known tool directories that have /bin/ but aren't build output.
                !lower.starts_with("/usr/")
                    && !lower.starts_with("/var/")
                    && !lower.contains("/.rustup/")
                    && !lower.contains("/.cargo/")
                    && (lower.contains("/build/")
                        || lower.contains("/out/")
                        || lower.contains("/obj/")
                        || lower.contains("/artifacts/")
                        || lower.ends_with("/build")
                        || lower.ends_with("/out"))
            },
            category: Category::BuildCache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Build output directory — regenerated by build tools",
        },
        Rule {
            name: "cache-package-npm",
            matches: |_, lower| {
                lower.contains("/.npm/_cacache/")
                    || lower.contains("/.cache/yarn/")
                    || lower.contains("/.cache/pnpm/")
            },
            category: Category::PackageCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "JavaScript package cache — redownloaded on npm install",
        },
        Rule {
            name: "cache-package-pip",
            matches: |_, lower| lower.contains("/.cache/pip/") || lower.contains("/.cache/uv/"),
            category: Category::PackageCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Python package cache — redownloaded from PyPI",
        },
        Rule {
            name: "cache-package-maven",
            matches: |_, lower| lower.contains("/.m2/repository/"),
            category: Category::PackageCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Maven repository — redownloaded on next build",
        },
        Rule {
            name: "cache-browser",
            matches: |_, lower| {
                lower.contains("/.cache/mozilla/")
                    || lower.contains("/.cache/chromium/")
                    || lower.contains("/.cache/google-chrome/")
                    || lower.contains("/bravesoftware/")
                    || lower.contains("/brave-browser/")
                    || lower.contains("/.cache/edge/")
                    || (lower.contains("/.mozilla/firefox/") && lower.contains("/cache"))
            },
            category: Category::BrowserCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Browser cache — rebuilt automatically while browsing",
        },
        Rule {
            name: "cache-user",
            matches: |_, lower| lower.contains("/.cache/") || lower.ends_with("/.cache"),
            category: Category::Cache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Application cache — applications regenerate when needed",
        },
        Rule {
            name: "cache-tmp",
            matches: |_, lower| lower.starts_with("/tmp/"),
            category: Category::TemporaryFile,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Temporary file — designed to be cleaned",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 7: Application-specific caches
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "app-docker",
            matches: |_, lower| {
                lower.contains("/docker/")
                    || lower.contains("/containerd/")
                    || lower.contains("/.local/share/docker/")
            },
            category: Category::DockerStorage,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Docker image layers and build cache",
        },
        Rule {
            name: "app-ai-models",
            matches: |_, lower| {
                lower.contains("/.cache/huggingface/")
                    || lower.contains("/.ollama/models/")
                    || lower.contains("/.cache/torch/")
            },
            category: Category::AIModelCache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "AI model cache — can be redownloaded from HuggingFace/Ollama",
        },
        Rule {
            name: "app-steam-shader",
            matches: |_, lower| {
                lower.contains("/.cache/dxvk-cache")
                    || lower.contains("/.cache/vkd3d")
                    || lower.contains("/.cache/mesa")
            },
            category: Category::GameData,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "GPU shader cache — regenerated when games launch",
        },
        Rule {
            name: "app-steam-compat",
            matches: |_, lower| lower.contains("/compatdata/"),
            category: Category::GameData,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Proton compatibility data — Steam reinstalls when needed",
        },
        Rule {
            name: "app-discord",
            matches: |_, lower| lower.contains("/discord/") && lower.contains("/cache/"),
            category: Category::Cache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Discord media cache — redownloaded as needed",
        },
        Rule {
            name: "app-vscode",
            matches: |_, lower| lower.contains("/Code/") && lower.contains("/cache/"),
            category: Category::Cache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "VS Code cache — extensions re-downloaded, settings preserved",
        },
        Rule {
            name: "app-trash",
            matches: |_, lower| {
                lower.contains("/.local/share/Trash/") || lower.contains("/.Trash/")
            },
            category: Category::Cache,
            risk_level: RiskLevel::Low,
            regenerable: false,
            reason: "Desktop trash — already deleted files; restore before cleaning if needed",
        },
        Rule {
            name: "app-cargo-registry",
            matches: |_, lower| {
                lower.contains("/.cargo/registry/") || lower.ends_with("/.cargo/registry")
            },
            category: Category::DownloadedArtifact,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Cargo crate registry — redownloaded on next build, but large",
        },
        Rule {
            name: "app-node-modules",
            matches: |_, lower| lower.contains("/node_modules/"),
            category: Category::DownloadedArtifact,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Node.js dependencies — reinstalled from package.json, but large and slow to restore",
        },
        Rule {
            name: "app-gradle",
            matches: |_, lower| lower.contains("/.gradle/") && !lower.contains("/caches/"),
            category: Category::BuildCache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Gradle wrapper and build cache",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 8: Application data — review before cleaning
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "data-local-share",
            matches: |_, lower| lower.contains("/.local/share/"),
            category: Category::ApplicationData,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "User application data — may contain saved states and databases",
        },
        Rule {
            name: "data-browser-profile",
            matches: |_, lower| {
                (lower.contains("/.mozilla/firefox/") && !lower.contains("/cache"))
                    || (lower.contains("/.config/google-chrome/") && !lower.contains("/cache"))
            },
            category: Category::ApplicationData,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Browser profile — bookmarks, passwords, extensions",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 8.4: Rustup toolchain management
        // REDIRECTED: All /.rustup/ paths are now caught by the
        // rustup-any rule at Layer 3.5b (before user-downloads),
        // which prevents ~/.rustup/downloads/ from being misclassified.
        // The specific rustup-home and rustup-toolchains-dir rules
        // are no longer needed here — rustup-any handles all subpaths.
        // ═══════════════════════════════════════════════════════
        // ═══════════════════════════════════════════════════════
        // LAYER 8.5: Developer workspace — build manifests, source dirs, scripts
        // Must come before config-file-ext to prevent Cargo.toml,
        // package.json, go.mod from being classified as generic
        // Application Configuration.
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "rust-cargo-toml",
            matches: |path, lower| {
                // Cargo.toml inside /.rustup/ is part of the toolchain, not a project manifest
                !lower.contains("/.rustup/")
                    && path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml")
            },
            category: Category::BuildManifest,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Rust package manifest — defines project metadata, dependencies, and build configuration",
        },
        Rule {
            name: "rust-cargo-lock",
            matches: |path, _| {
                path.file_name().and_then(|n| n.to_str()) == Some("Cargo.lock")
            },
            category: Category::DependencyLockfile,
            risk_level: RiskLevel::High,
            regenerable: true,
            reason: "Rust dependency lockfile — ensures reproducible builds by pinning dependency versions",
        },
        Rule {
            name: "node-package-json",
            matches: |path, _| {
                path.file_name().and_then(|n| n.to_str()) == Some("package.json")
            },
            category: Category::BuildManifest,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Node.js package manifest — defines project metadata, dependencies, and scripts",
        },
        Rule {
            name: "node-package-lock",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == "package-lock.json"
                    || name == "yarn.lock"
                    || name == "pnpm-lock.yaml"
            },
            category: Category::DependencyLockfile,
            risk_level: RiskLevel::High,
            regenerable: true,
            reason: "Node.js dependency lockfile — ensures reproducible installs by pinning versions",
        },
        Rule {
            name: "go-mod",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == "go.mod" || name == "go.sum"
            },
            category: Category::BuildManifest,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Go module definition — defines module path and dependency requirements",
        },
        Rule {
            name: "source-dir",
            matches: |_, lower| {
                // Match src/ as a project source directory.
                // Exclude /usr/src (system source), /.rustup/ (toolchain source),
                // and /.cargo/ (registry source) — these are NOT project source.
                !lower.contains("/.rustup/")
                    && !lower.contains("/.cargo/")
                    && !lower.starts_with("/usr/src")
                    && ((lower.ends_with("/src") || lower == "src")
                        || lower.contains("/src/"))
            },
            category: Category::SourceDirectory,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Source code directory — contains project source files",
        },
        Rule {
            name: "shell-script",
            matches: |path, _| path.extension().and_then(|e| e.to_str()) == Some("sh"),
            category: Category::ShellScript,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "Shell script — automation, build, or deployment script",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 8.6: Rustup toolchain management (MOVED to 8.4 above)
        // This section intentionally left empty — rules moved earlier
        // to prevent rustup paths from being misclassified by
        // source-dir, manifest, and config rules.
        // ═══════════════════════════════════════════════════════
        // ═══════════════════════════════════════════════════════
        // LAYER 9: Config files by extension (generic fallback)
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "config-file-ext",
            matches: |path, _| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".toml")
                    || name.ends_with(".yaml")
                    || name.ends_with(".yml")
                    || name.ends_with(".json")
                    || name.ends_with(".ini")
                    || name.ends_with(".conf")
                    || name.ends_with(".cfg")
            },
            category: Category::ApplicationConfiguration,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "Configuration file — settings customized by user or application",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_priority_system_first() {
        let rules = rule_database();
        // First rules should be system protection
        assert_eq!(rules[0].name, "sys-bin-usr");
        assert_eq!(rules[1].name, "sys-bin-root");
        assert_eq!(rules[2].name, "sys-etc");
    }

    #[test]
    fn test_system_binary_not_cache() {
        let rules = rule_database();
        let brave_rule = rules.iter().find(|r| r.name == "cache-browser").unwrap();
        // brave binary path should NOT match browser cache rule
        assert!(!(brave_rule.matches)(
            Path::new("/usr/bin/brave"),
            &"/usr/bin/brave".to_lowercase()
        ));
        // But brave cache SHOULD match
        assert!((brave_rule.matches)(
            Path::new("/home/user/.cache/BraveSoftware/Brave-Browser/Cache/data_0"),
            &"/home/user/.cache/bravesoftware/brave-browser/cache/data_0".to_lowercase()
        ));
    }

    #[test]
    fn test_etc_never_cache() {
        let rules = rule_database();
        let cache_rule = rules.iter().find(|r| r.name == "cache-user").unwrap();
        assert!(!(cache_rule.matches)(
            Path::new("/etc/environment"),
            &"/etc/environment".to_lowercase()
        ));
    }

    #[test]
    fn test_home_root_not_cleanable() {
        let rules = rule_database();
        let home_rule = rules.iter().find(|r| r.name == "home-root").unwrap();
        assert!((home_rule.matches)(
            Path::new("/home/user"),
            &"/home/user".to_lowercase()
        ));
        assert!(!(home_rule.matches)(
            Path::new("/home/user/.cache"),
            &"/home/user/.cache".to_lowercase()
        ));
    }

    #[test]
    fn test_ssh_is_protected() {
        let rules = rule_database();
        let ssh_rule = rules.iter().find(|r| r.name == "sec-ssh").unwrap();
        assert!((ssh_rule.matches)(
            Path::new("/home/user/.ssh/id_ed25519"),
            &"/home/user/.ssh/id_ed25519".to_lowercase()
        ));
        assert_eq!(ssh_rule.category, Category::SecurityCredential);
        assert_eq!(ssh_rule.risk_level, RiskLevel::Critical);
    }
}
