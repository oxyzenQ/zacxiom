// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Structured rule database — replaces giant if/else chains.
//!
//! Rules are ordered by priority. First match wins.
//! Each rule specifies a path pattern, resulting category, and risk level.
//!
//! v7: Rules carry artifact intelligence — ownership, regeneration,
//! dependency, and deletion impact metadata.

use super::types::{Category, RiskLevel};
use std::path::Path;
use std::sync::OnceLock;

/// A single classification rule.
///
/// v7: Enriched with artifact intelligence fields.
pub struct Rule {
    pub name: &'static str,
    /// Match logic: returns true if this rule applies to the given path.
    pub matches: fn(&Path, &str) -> bool,
    pub category: Category,
    pub risk_level: RiskLevel,
    pub regenerable: bool,
    pub reason: &'static str,
    // ── v7: Artifact Intelligence fields ──────────────────────
    /// Who created this artifact? (e.g. "Cargo", "Rustup", "npm", "Browser")
    pub created_by: &'static str,
    /// How to regenerate this artifact? (e.g. "cargo build", "rustup toolchain install")
    pub regenerated_by: &'static str,
    /// What does this artifact depend on? (e.g. "Cargo.toml", "package.json")
    pub depends_on: &'static str,
    /// What happens if this artifact is deleted?
    pub deletion_impact: &'static str,
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
            created_by: "Package manager (apt/pacman/dnf)",
            regenerated_by: "Package manager reinstall",
            depends_on: "Package repository index",
            deletion_impact: "Application stops working. May require full reinstall.",
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
            created_by: "OS installer / package manager",
            regenerated_by: "OS reinstall or package reinstall",
            depends_on: "Package repository",
            deletion_impact: "Critical system failure. OS may become unbootable.",
        },
        Rule {
            name: "sys-etc",
            matches: |_, lower| lower.starts_with("/etc/") || lower == "/etc",
            category: Category::SystemConfiguration,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "System-wide configuration — controls OS behavior",
            created_by: "Package installations / system administrator",
            regenerated_by: "Package reinstall (defaults) or manual restore from backup",
            depends_on: "Installed packages",
            deletion_impact: "System services misconfigured or fail to start.",
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
            created_by: "Package manager (apt/pacman/dnf)",
            regenerated_by: "Package reinstall",
            depends_on: "Package repository",
            deletion_impact: "Many applications fail to run. System may become unstable.",
        },
        Rule {
            name: "sys-boot",
            matches: |_, lower| lower.starts_with("/boot/") || lower == "/boot",
            category: Category::SystemData,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "Boot files — deletion prevents system startup",
            created_by: "Kernel installation / bootloader setup",
            regenerated_by: "Kernel reinstall + bootloader repair (recovery media)",
            depends_on: "Installed kernel packages",
            deletion_impact: "System cannot boot. Requires recovery media to repair.",
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
            created_by: "Kernel (at runtime)",
            regenerated_by: "Reboot (kernel recreates automatically)",
            depends_on: "None (in-memory, not on disk)",
            deletion_impact: "Kernel panics or device malfunction possible.",
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
            created_by: "Manual installation or third-party package",
            regenerated_by: "Reinstall from original source",
            depends_on: "Original installer or package",
            deletion_impact: "Third-party application stops working.",
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
            created_by: "System (useradd) or OS installer",
            regenerated_by: "Not regenerable — user must recreate from scratch",
            depends_on: "None",
            deletion_impact: "All personal files, projects, configs, and data permanently lost.",
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
            created_by: "Package manager (cargo install / pip install / npm -g)",
            regenerated_by: "Reinstall via package manager (cargo install / npm -g)",
            depends_on: "Package registry (crates.io / npmjs.com)",
            deletion_impact: "Installed tools stop working. Must reinstall individually.",
        },
        Rule {
            name: "user-bin-dir",
            matches: |_, lower| lower.ends_with("/.local/bin") || lower.ends_with("/.cargo/bin"),
            category: Category::SystemBinary,
            risk_level: RiskLevel::Critical,
            regenerable: false,
            reason: "User software directory — contains locally installed executables",
            created_by: "User (manual install / cargo install / pipx)",
            regenerated_by: "Reinstall all tools individually",
            depends_on: "Various package registries",
            deletion_impact: "All user-installed tools lost. Must reinstall everything.",
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

            created_by: "Rustup",

            regenerated_by: "rustup toolchain install",

            depends_on: "rustup",

            deletion_impact: "All installed Rust toolchains lost. Must reinstall via rustup.",
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

            created_by: "Rustup",

            regenerated_by: "rustup toolchain install",

            depends_on: "rustup",

            deletion_impact: "Toolchain files lost. Reinstall required.",
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

            created_by: "Git",

            regenerated_by: "git clone / git fetch",

            depends_on: "Remote repository",

            deletion_impact: "Repository history corrupted. May lose uncommitted work.",
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
            created_by: "SVN/Mercurial",
            regenerated_by: "Fresh checkout from remote",
            depends_on: "Remote repository",
            deletion_impact: "Working copy corrupted. Must check out fresh.",
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

            created_by: "User/ssh-keygen",

            regenerated_by: "ssh-keygen (creates new keys, old ones lost)",

            depends_on: "None",

            deletion_impact: "Permanent access loss. Cannot authenticate to remote systems.",
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
            created_by: "User (gpg --gen-key)",
            regenerated_by: "Not regenerable — creates new keys, old ones lost forever",
            depends_on: "None",
            deletion_impact: "Permanent loss of encryption identity. Cannot decrypt old messages.",
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
            created_by: "User (ssh-keygen / openssl)",
            regenerated_by: "Not regenerable — creates new keys, old access lost",
            depends_on: "None",
            deletion_impact: "Permanent access loss to remote systems and services.",
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
            created_by: "User",
            regenerated_by: "Not regenerable",
            depends_on: "None",
            deletion_impact: "Desktop files permanently lost.",
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
            created_by: "User",
            regenerated_by: "Not regenerable",
            depends_on: "None",
            deletion_impact: "Personal documents permanently lost.",
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
            created_by: "User",
            regenerated_by: "Not regenerable (unless backed up)",
            depends_on: "None",
            deletion_impact: "Music/media files permanently lost.",
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
            created_by: "User / Camera / Screenshot tool",
            regenerated_by: "Not regenerable (unless backed up)",
            depends_on: "None",
            deletion_impact: "Photos and images permanently lost.",
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
            created_by: "User / Recording software",
            regenerated_by: "Not regenerable (unless backed up)",
            depends_on: "None",
            deletion_impact: "Video files permanently lost.",
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

            created_by: "User/Browser",

            regenerated_by: "Not regenerable",

            depends_on: "None",

            deletion_impact: "Downloaded files permanently lost.",
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
            created_by: "User / shell initialization",
            regenerated_by: "Not regenerable — must recreate manually",
            depends_on: "None",
            deletion_impact: "Shell environment resets to defaults. All aliases, PATH customizations, and prompt settings lost.",
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
            created_by: "User (git config)",
            regenerated_by: "git config (reconfigure manually)",
            depends_on: "None",
            deletion_impact: "Git identity, aliases, and credentials lost.",
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
            created_by: "User / deployment tool",
            regenerated_by: "Not regenerable — secrets and API keys must be reissued",
            depends_on: "None",
            deletion_impact: "Environment variables reset. Applications may fail to configure.",
        },
        // v8.3.1: Explicit ~/.config directory classification.
        // Must come before config-app (which requires /.config/ with trailing slash)
        // and before any generic rules that would misclassify it as UserHomeRoot.
        Rule {
            name: "config-dir-root",
            matches: |_, lower| lower.ends_with("/.config") || lower.ends_with("/.config/"),
            category: Category::ApplicationConfiguration,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Configuration directory — contains application settings and preferences",
            created_by: "Applications (on first run / settings save)",
            regenerated_by: "Applications recreate defaults on next launch — customizations lost",
            depends_on: "Applications",
            deletion_impact: "All custom settings and preferences lost. Apps reset to factory defaults.",
        },
        Rule {
            name: "config-app",
            matches: |_, lower| lower.contains("/.config/"),
            category: Category::ApplicationConfiguration,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "Application settings — apps recreate defaults but customizations are lost",
            created_by: "Application (on first run / settings save)",
            regenerated_by: "Application recreates defaults on next launch",
            depends_on: "Application",
            deletion_impact: "All custom settings and preferences lost. Apps reset to factory defaults.",
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

            created_by: "Cargo",

            regenerated_by: "cargo fetch",

            depends_on: "Cargo.toml",

            deletion_impact: "Git checkouts removed. Must re-clone on next build.",
        },
        Rule {
            name: "cache-build-gradle",
            matches: |_, lower| lower.contains("/.gradle/caches/"),
            category: Category::BuildCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Gradle build cache — redownloaded on next build",
            created_by: "Gradle",
            regenerated_by: "gradle build",
            depends_on: "build.gradle + source code + dependency repositories",
            deletion_impact: "Next Gradle build slower — must re-download and recompile dependencies.",
        },
        Rule {
            name: "cache-build-target",
            matches: |_, lower| {
                // Match bare "target", "project/target", "/target/" subdirectories,
                // AND bare "target/" prefix for children like target/debug, target/release.
                // Exclude /usr/src (system territory) and /var/ (system data).
                lower == "target"
                    || lower.ends_with("/target")
                    || lower.starts_with("target/")
                    || lower.starts_with("./target/")
                    || (lower.contains("/target/")
                        && !lower.starts_with("/usr/")
                        && !lower.starts_with("/var/"))
            },
            category: Category::BuildCache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Rust build artifacts — regenerated by cargo build",

            created_by: "Cargo",

            regenerated_by: "cargo build",

            depends_on: "Cargo.toml + source code",

            deletion_impact: "Build output removed. Next build starts from scratch.",
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
            created_by: "Next.js / Nuxt / webpack / bundler",
            regenerated_by: "npm run build / yarn build",
            depends_on: "package.json + source code + node_modules",
            deletion_impact: "Build output removed. Next build regenerates from source.",
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
            created_by: "Build tool (make / cmake / gradle / msbuild)",
            regenerated_by: "Build command (make / cmake --build / gradle build)",
            depends_on: "Build configuration + source code",
            deletion_impact: "Build output removed. Next build regenerates.",
        },
        Rule {
            name: "cache-package-npm",
            matches: |_, lower| {
                lower.contains("/.npm/_cacache/")
                    || lower.contains("/.cache/yarn/")
                    || lower.contains("/.cache/pnpm/")
            },
            category: Category::CacheRegistry,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "JavaScript package cache — redownloaded on npm install",
            created_by: "npm / yarn / pnpm",
            regenerated_by: "npm install / yarn install / pnpm install",
            depends_on: "npm registry (registry.npmjs.org)",
            deletion_impact: "Next install must re-download all packages from registry.",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 6.5: npm ecosystem — npx cache, npm cache artifacts
        // MUST come before generic cache/config rules to prevent
        // ~/.npm/* from being classified as Unknown or ApplicationData.
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "npm-npx-cache",
            matches: |_, lower| {
                lower.contains("/.npm/_npx/")
                    || lower.ends_with("/.npm/_npx")
            },
            category: Category::DownloadedArtifact,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "npx package cache — downloaded on-demand by npx, regenerable via re-download",
            created_by: "npx (Node.js package runner)",
            regenerated_by: "npx <package> (auto-downloads on next use)",
            depends_on: "npm registry (registry.npmjs.org)",
            deletion_impact: "Next npx invocation re-downloads packages from npm registry. No data loss.",
        },
        Rule {
            name: "npm-logs",
            matches: |_, lower| {
                lower.contains("/.npm/_logs/")
                    || lower.ends_with("/.npm/_logs")
            },
            category: Category::Cache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "npm debug logs — diagnostic output from npm operations, safe to clean",
            created_by: "npm (Node.js package manager)",
            regenerated_by: "Any npm command (logs generated on next npm operation)",
            depends_on: "npm usage",
            deletion_impact: "Debug logs removed. No functional impact — new logs created on next npm command.",
        },
        Rule {
            name: "npm-cache-generic",
            matches: |_, lower| {
                // Catch ~/.npm/ subdirectories not covered by specific rules
                // Exclude ~/.npm/_cacache/ which is already handled as CacheRegistry
                lower.contains("/.npm/") && !lower.contains("/_cacache/")
            },
            category: Category::ApplicationData,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "npm metadata and state — package manager configuration and cache",
            created_by: "npm (Node.js package manager)",
            regenerated_by: "npm install (metadata rebuilt from registry)",
            depends_on: "npm registry (registry.npmjs.org)",
            deletion_impact: "npm configuration and metadata lost. Can be rebuilt via npm install.",
        },
        Rule {
            name: "cache-package-pip",
            matches: |_, lower| lower.contains("/.cache/pip/") || lower.contains("/.cache/uv/"),
            category: Category::CacheRegistry,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Python package cache — redownloaded from PyPI",
            created_by: "pip / uv",
            regenerated_by: "pip install / uv pip install",
            depends_on: "PyPI registry (pypi.org)",
            deletion_impact: "Next install must re-download packages from PyPI.",
        },
        Rule {
            name: "cache-package-maven",
            matches: |_, lower| lower.contains("/.m2/repository/"),
            category: Category::CacheRegistry,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Maven repository — redownloaded on next build",
            created_by: "Maven / Gradle",
            regenerated_by: "mvn compile / gradle build",
            depends_on: "Maven Central / Gradle Plugin Portal",
            deletion_impact: "Next build must re-download all dependencies from repositories.",
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

            created_by: "Browser",

            regenerated_by: "Browsing websites",

            depends_on: "None",

            deletion_impact: "Websites load slower on first visit. No data loss.",
        },
        Rule {
            name: "cache-user",
            matches: |_, lower| lower.contains("/.cache/") || lower.ends_with("/.cache"),
            category: Category::Cache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Application cache — applications regenerate when needed",

            created_by: "Application",

            regenerated_by: "Application usage",

            depends_on: "None",

            deletion_impact: "Application may run slower until cache rebuilds.",
        },
        Rule {
            name: "cache-tmp",
            matches: |_, lower| lower.starts_with("/tmp/"),
            category: Category::TemporaryFile,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Temporary file — designed to be cleaned",

            created_by: "System/Application",

            regenerated_by: "Not regenerable (temporary)",

            depends_on: "None",

            deletion_impact: "No impact. Temporary files are safe to remove.",
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
            created_by: "Docker / Podman",
            regenerated_by: "docker pull / docker build",
            depends_on: "Dockerfile / container registry",
            deletion_impact: "Images must be re-pulled / rebuilt from Dockerfile or registry.",
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
            created_by: "HuggingFace Hub / Ollama / PyTorch",
            regenerated_by: "Model re-download from HuggingFace / Ollama / PyTorch Hub",
            depends_on: "Internet access + model repository",
            deletion_impact: "Models must be re-downloaded. Checkpoints stored elsewhere are unaffected.",
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
            created_by: "DXVK / VKD3D / Mesa drivers",
            regenerated_by: "Game launch (GPU drivers recompile shaders)",
            depends_on: "Installed games",
            deletion_impact: "Games take longer to launch first time. Shader compilation happens in background.",
        },
        Rule {
            name: "app-steam-compat",
            matches: |_, lower| lower.contains("/compatdata/"),
            category: Category::GameData,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Proton compatibility data — Steam reinstalls when needed",
            created_by: "Steam / Proton",
            regenerated_by: "Steam recreates on next game launch",
            depends_on: "Steam + installed game",
            deletion_impact: "Proton prefixes lost. Game saves within compatdata may be lost — check first.",
        },
        Rule {
            name: "app-discord",
            matches: |_, lower| lower.contains("/discord/") && lower.contains("/cache/"),
            category: Category::Cache,
            risk_level: RiskLevel::Minimal,
            regenerable: true,
            reason: "Discord media cache — redownloaded as needed",
            created_by: "Discord",
            regenerated_by: "Discord usage (images/media re-downloaded on view)",
            depends_on: "None",
            deletion_impact: "Images and media reload when viewed. No data loss.",
        },
        Rule {
            name: "app-vscode",
            matches: |_, lower| lower.contains("/Code/") && lower.contains("/cache/"),
            category: Category::Cache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "VS Code cache — extensions re-downloaded, settings preserved",
            created_by: "VS Code / Cursor / Windsurf",
            regenerated_by: "IDE restart (extensions and caches rebuild)",
            depends_on: "Extension marketplace",
            deletion_impact: "Extensions re-download. Settings and workspace state preserved.",
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
            created_by: "User (file deletion)",
            regenerated_by: "Not regenerable — files were intentionally deleted",
            depends_on: "None",
            deletion_impact: "Trashed files permanently removed. Check for accidentally deleted files first.",
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

            created_by: "Cargo",

            regenerated_by: "cargo fetch / cargo build",

            depends_on: "Cargo.toml",

            deletion_impact: "Next build must re-download all dependencies. Offline builds will fail.",
        },
        Rule {
            name: "app-node-modules",
            matches: |p, lower| {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                name == "node_modules" || lower.contains("/node_modules/")
            },
            category: Category::DownloadedArtifact,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Node.js dependencies — reinstalled from package.json, but large and slow to restore",

            created_by: "npm/yarn/pnpm",

            regenerated_by: "npm install / yarn install",

            depends_on: "package.json",

            deletion_impact: "Project dependencies removed. Must re-install before development.",
        },
        Rule {
            name: "app-gradle",
            matches: |_, lower| lower.contains("/.gradle/") && !lower.contains("/caches/"),
            category: Category::BuildCache,
            risk_level: RiskLevel::Low,
            regenerable: true,
            reason: "Gradle wrapper and build cache",
            created_by: "Gradle",
            regenerated_by: "gradle build / gradle wrapper",
            depends_on: "build.gradle + Gradle distribution",
            deletion_impact: "Gradle wrapper and daemon data lost. Re-downloaded on next build.",
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
            created_by: "Applications",
            regenerated_by: "Not automatically regenerable",
            depends_on: "Application",
            deletion_impact: "Application saved states and databases may be permanently lost.",
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
            created_by: "Browser (Firefox / Chrome)",
            regenerated_by: "Not regenerable — unless synced to cloud account",
            depends_on: "Browser sync account (if enabled)",
            deletion_impact: "Bookmarks, saved passwords, extensions, and browsing history permanently lost.",
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

            created_by: "User/Developer",

            regenerated_by: "Not regenerable",

            depends_on: "None",

            deletion_impact: "Project definition lost. Build system will fail.",
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
            created_by: "Cargo",
            regenerated_by: "cargo update / cargo generate-lockfile",
            depends_on: "Cargo.toml",
            deletion_impact: "Dependency versions may change on next build. Reproducibility lost.",
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
            created_by: "npm init / Developer",
            regenerated_by: "Not regenerable — must recreate manually",
            depends_on: "None",
            deletion_impact: "Project definition lost. Dependencies, scripts, and metadata must be recreated.",
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
            created_by: "npm / yarn / pnpm",
            regenerated_by: "npm install / yarn install / pnpm install",
            depends_on: "package.json",
            deletion_impact: "Dependency versions may change on next install. Team reproducibility lost.",
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
            created_by: "go mod init / Developer",
            regenerated_by: "go mod init + go mod tidy (recreates from scratch)",
            depends_on: "None",
            deletion_impact: "Go module definition lost. Must recreate module path and re-add dependencies.",
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

            created_by: "User/Developer",

            regenerated_by: "Not regenerable",

            depends_on: "None",

            deletion_impact: "Source code permanently lost.",
        },
        Rule {
            name: "shell-script",
            matches: |path, _| path.extension().and_then(|e| e.to_str()) == Some("sh"),
            category: Category::ProjectAsset,
            risk_level: RiskLevel::Moderate,
            regenerable: false,
            reason: "Shell script — automation, build, or deployment script",
            created_by: "Developer",
            regenerated_by: "Not regenerable — must rewrite from scratch",
            depends_on: "None",
            deletion_impact: "Automation and build scripts lost. Workflows may break.",
        },
        // ═══════════════════════════════════════════════════════
        // LAYER 8.6: Rustup toolchain management (MOVED to 8.4 above)
        // This section intentionally left empty — rules moved earlier
        // to prevent rustup paths from being misclassified by
        // source-dir, manifest, and config rules.
        // ═══════════════════════════════════════════════════════
        // ═══════════════════════════════════════════════════════
        // LAYER 8.7: Python build manifest — before generic .toml rule
        // pyproject.toml defines project metadata, dependencies, and build system.
        // Must classify as BuildManifest, not ApplicationConfiguration.
        // ═══════════════════════════════════════════════════════
        Rule {
            name: "python-pyproject",
            matches: |path, _| {
                path.file_name().and_then(|n| n.to_str()) == Some("pyproject.toml")
            },
            category: Category::BuildManifest,
            risk_level: RiskLevel::High,
            regenerable: false,
            reason: "Python package manifest — defines project metadata, dependencies, and build system",
            created_by: "Developer / poetry init / hatch new",
            regenerated_by: "Not regenerable — must recreate manually or restore from VCS",
            depends_on: "None",
            deletion_impact: "Project definition lost. Dependencies, scripts, and build configuration must be recreated.",
        },
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
            created_by: "Application / User",
            regenerated_by: "Application recreates defaults on launch",
            depends_on: "Application",
            deletion_impact: "Custom config settings lost. Application resets to defaults.",
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
