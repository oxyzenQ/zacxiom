// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Rule database — all classification rules in priority order.

use super::Rule;
use crate::engine::types::{Category, RiskLevel};

pub(crate) fn build_protected_rules() -> Vec<Rule> {
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
    ]
}
