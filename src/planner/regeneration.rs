// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cleanup Planner — Ecosystem-aware regeneration recommendations.

use std::path::Path;

use crate::discovery::Ecosystem;
use crate::engine::{Category, ClassificationResult};
use crate::ownership;

/// Detect the Node.js package manager from project manifest files.
pub(crate) fn detect_node_pm_from_manifests(manifests: &[std::path::PathBuf]) -> &'static str {
    for m in manifests {
        let name = m.file_name().and_then(|n| n.to_str()).unwrap_or("");
        match name {
            "pnpm-lock.yaml" => return "pnpm",
            "yarn.lock" => return "yarn",
            "package-lock.json" => return "npm",
            _ => {}
        }
    }
    "npm"
}

/// Build ecosystem-aware recommendations.
/// Prefers ecosystem commands over raw `rm -rf`.
/// Returns (recommendation, reason, regeneration, suggested_commands).
pub(crate) fn build_ecosystem_recommendation(
    path: &Path,
    eng: &ClassificationResult,
    ownership: &Option<ownership::OwnershipMatch>,
) -> (String, String, String, Vec<String>) {
    let ecosystem = ownership.as_ref().map(|om| om.project.ecosystem);
    let path_str = path.to_string_lossy().to_lowercase();
    let path_last = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // ── P2: Project root / CRITICAL — suggest child cleanup, never root deletion ──
    let is_project_root = ownership.as_ref().is_some_and(|om| {
        matches!(
            om.evidence.ownership_type,
            ownership::OwnershipType::ProjectRoot
        )
    }) || matches!(
        eng.category,
        Category::ProjectWorkspace | Category::SourceDirectory
    );

    if is_project_root && !eng.category.is_cleanable() {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Clean generated artifacts instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => {
                let pm = ownership.as_ref().map_or("npm", |om| {
                    detect_node_pm_from_manifests(&om.project.manifests)
                });
                (
                    "Clean dependency installations instead.".into(),
                    "Project root contains non-regenerable source code.".into(),
                    String::new(),
                    vec![format!("{pm} install")],
                )
            }
            Some(Ecosystem::Python) => (
                "Clean virtual environment and cache instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["python -m venv .venv".into()],
            ),
            Some(Ecosystem::Go) => (
                "Clean build cache instead.".into(),
                "Project root contains non-regenerable source code.".into(),
                String::new(),
                vec!["go clean -cache".into()],
            ),
            None => (
                "Clean safe subdirectories instead.".into(),
                "This directory contains non-regenerable content.".into(),
                String::new(),
                Vec::new(),
            ),
        };
    }

    // ── Build artifacts ──
    if eng.category == Category::BuildCache {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Remove build artifacts.".into(),
                "Compiled binaries and intermediate objects are fully regenerable from source."
                    .into(),
                "cargo build".into(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => {
                let pm = ownership.as_ref().map_or("npm", |om| {
                    detect_node_pm_from_manifests(&om.project.manifests)
                });
                let cmd = if path_last == "node_modules" {
                    format!("{pm} install")
                } else {
                    format!("{pm} run build")
                };
                (
                    "Remove dependencies and reinstall if needed.".into(),
                    "Installed packages are declared in package.json and re-downloadable.".into(),
                    format!("{pm} install"),
                    vec![cmd],
                )
            }
            Some(Ecosystem::Go) => (
                "Remove Go build cache.".into(),
                "Compiled binaries are fully regenerable from source.".into(),
                "go build ./...".into(),
                vec!["go clean -cache".into()],
            ),
            _ => {
                let regen = if !eng.regenerated_by.is_empty() {
                    eng.regenerated_by.clone()
                } else {
                    "Rebuild the project".into()
                };
                let reason = if !eng.deletion_impact.is_empty() {
                    eng.deletion_impact.clone()
                } else {
                    "Build output is regenerable from source and dependencies.".into()
                };
                ("Remove build output.".into(), reason, regen, Vec::new())
            }
        };
    }

    // ── Generated content ──
    if eng.category == Category::GeneratedContent {
        let regen = if !eng.regenerated_by.is_empty() {
            eng.regenerated_by.clone()
        } else {
            "Regenerate from source".into()
        };
        return (
            "Remove generated content.".into(),
            "Auto-generated from source — no manual work lost.".into(),
            regen,
            Vec::new(),
        );
    }

    // ── Cache categories ──
    if matches!(
        eng.category,
        Category::Cache | Category::BrowserCache | Category::CacheRegistry
    ) {
        let recommendation = match eng.category {
            Category::BrowserCache => "Clear browser cache.".into(),
            Category::CacheRegistry => "Remove package download cache.".into(),
            _ => "Clear application cache.".into(),
        };
        let reason = match eng.category {
            Category::BrowserCache => {
                "Web resources are re-downloaded automatically while browsing.".into()
            }
            Category::CacheRegistry => {
                "Package metadata is rebuilt on next package operation.".into()
            }
            _ => "Disposable data — applications regenerate it on next use.".into(),
        };
        let regen = if !eng.regenerated_by.is_empty() {
            eng.regenerated_by.clone()
        } else {
            "Automatic — regenerated on next use".into()
        };
        return (recommendation, reason, regen, Vec::new());
    }

    // ── Temporary files ──
    if eng.category == Category::TemporaryFile {
        match ecosystem {
            Some(Ecosystem::Node) => match path_last.as_str() {
                "node_modules" => {
                    let pm = ownership.as_ref().map_or("npm", |om| {
                        detect_node_pm_from_manifests(&om.project.manifests)
                    });
                    return (
                        "Remove dependencies and reinstall.".into(),
                        "Packages are declared in package.json and re-downloadable.".into(),
                        format!("{pm} install"),
                        vec![format!("{pm} install")],
                    );
                }
                "dist" => {
                    let pm = ownership.as_ref().map_or("npm", |om| {
                        detect_node_pm_from_manifests(&om.project.manifests)
                    });
                    return (
                        "Remove build output.".into(),
                        "Build output is regenerable from source.".into(),
                        format!("{pm} run build"),
                        vec![format!("{pm} run build")],
                    );
                }
                _ => {}
            },
            Some(Ecosystem::Rust) if path_last.as_str() == "target" => {
                return (
                    "Remove build artifacts.".into(),
                    "Compiled binaries are fully regenerable from source.".into(),
                    "cargo build".into(),
                    vec!["cargo clean".into()],
                );
            }
            _ => {}
        }
        return (
            "Remove temporary files.".into(),
            "Designed by the OS and applications to be disposable.".into(),
            "Automatic — recreated by applications as needed".into(),
            Vec::new(),
        );
    }

    // ── Dependency sources ──
    if matches!(
        eng.category,
        Category::DependencySource | Category::DownloadedArtifact
    ) {
        return match ecosystem {
            Some(Ecosystem::Rust) => (
                "Remove downloaded dependency sources.".into(),
                "Crates are re-downloaded from crates.io on next build.".into(),
                "cargo build (re-downloads dependencies)".into(),
                vec!["cargo clean".into()],
            ),
            Some(Ecosystem::Node) => {
                let pm = ownership.as_ref().map_or("npm", |om| {
                    detect_node_pm_from_manifests(&om.project.manifests)
                });
                if path_last == "node_modules" {
                    (
                        "Remove dependencies and reinstall.".into(),
                        "Packages are declared in package.json and re-downloadable.".into(),
                        format!("{pm} install"),
                        vec![format!("{pm} install")],
                    )
                } else {
                    (
                        "Remove downloaded packages.".into(),
                        "Packages are re-downloadable from the npm registry.".into(),
                        format!("{pm} install (re-downloads packages)"),
                        vec![format!("{pm} cache clean --force")],
                    )
                }
            }
            Some(Ecosystem::Python) => (
                "Remove downloaded packages.".into(),
                "Packages are re-downloadable from PyPI.".into(),
                "pip install (re-downloads packages)".into(),
                Vec::new(),
            ),
            _ => (
                "Remove downloaded dependency sources.".into(),
                "Dependencies are re-downloadable from their registry.".into(),
                "Re-download via package manager".into(),
                Vec::new(),
            ),
        };
    }

    // ── Toolchain ──
    if matches!(
        eng.category,
        Category::ToolchainManager | Category::ToolchainInstallation
    ) {
        // Toolchains are NOT safe to auto-clean — recommend against deletion
        return (
            "Do not delete — installed development tooling.".into(),
            "Installed compiler/runtime — not auto-regenerated.".into(),
            "Reinstall via toolchain manager (rustup, nvm, etc.)".into(),
            Vec::new(),
        );
    }

    // ── Python-specific: __pycache__ ──
    if path_last == "__pycache__" || path_str.contains("/__pycache__/") {
        return (
            "Remove Python bytecode cache.".into(),
            "Bytecode is recompiled automatically when modules are imported.".into(),
            "Automatic".into(),
            vec!["python -m compileall".into()],
        );
    }

    // ── Application data ──
    if matches!(
        eng.category,
        Category::ApplicationData | Category::DockerStorage | Category::GameData
    ) {
        return (
            "Review before cleaning — may contain user data or state.".into(),
            "Application state may include preferences, saves, or session data.".into(),
            "Not easily regenerable — may require reconfiguration".into(),
            Vec::new(),
        );
    }

    // ── AI model cache ──
    if eng.category == Category::AIModelCache {
        return (
            "Remove AI model cache.".into(),
            "Downloaded model weights are re-downloadable from the model hub.".into(),
            "Re-download on next use".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: project-level ──
    if matches!(
        eng.category,
        Category::BuildManifest | Category::ProjectAsset
    ) {
        return (
            "Keep this file.".into(),
            "Project definition file — defines dependencies and build configuration.".into(),
            String::new(),
            Vec::new(),
        );
    }

    // ── Unsafe: security credentials ──
    if eng.category == Category::SecurityCredential {
        let path_str = path.to_string_lossy().to_lowercase();
        if path_str.contains(".ssh") {
            return (
                "Keep SSH credentials.".into(),
                "Authentication keys cannot be regenerated.".into(),
                "SSH access continues to work.".into(),
                Vec::new(),
            );
        }
        if path_str.contains(".gnupg") || path_str.contains("gpg") {
            return (
                "Keep encryption keys.".into(),
                "GPG keys and trust database cannot be regenerated.".into(),
                "Encrypted communications and signatures continue to work.".into(),
                Vec::new(),
            );
        }
        return (
            "Preserve authentication credentials.".into(),
            "Credentials cannot be recovered once deleted.".into(),
            "Authentication continues to work.".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: system ──
    if matches!(
        eng.category,
        Category::SystemBinary
            | Category::SystemConfiguration
            | Category::SystemData
            | Category::VirtualFilesystem
    ) {
        let reason = match eng.category {
            Category::SystemBinary => {
                "Operating system requires these binaries to function.".into()
            }
            Category::SystemConfiguration => {
                "System services depend on these configuration files.".into()
            }
            _ => "Operating system infrastructure — required for system stability.".into(),
        };
        return (
            "Preserve system infrastructure.".into(),
            reason,
            "OS reinstall required".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: configuration ──
    if matches!(
        eng.category,
        Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile
    ) {
        return match eng.category {
            Category::ShellConfiguration => (
                "Do not clean this directory.".into(),
                "Contains aliases, functions, and shell customizations.".into(),
                "Shell loses all user-defined behavior and prompt settings.".into(),
                Vec::new(),
            ),
            Category::ApplicationConfiguration => (
                "Do not clean this directory.".into(),
                "Contains application settings and preferences.".into(),
                "Application recreates default configuration. User customizations are lost.".into(),
                Vec::new(),
            ),
            Category::EnvironmentFile => (
                "Do not clean this file.".into(),
                "Contains environment variables and secrets.".into(),
                "Application recreates default environment. Secrets and overrides are lost.".into(),
                Vec::new(),
            ),
            _ => unreachable!(),
        };
    }

    // ── Unsafe: user content ──
    if matches!(
        eng.category,
        Category::UserDocument | Category::UserMedia | Category::UserDesktop
    ) {
        return (
            "Protect user-created content.".into(),
            "User-authored files are irreplaceable without a backup.".into(),
            "Irreplaceable without backup".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: installed software ──
    if eng.category == Category::InstalledSoftware {
        return (
            "Preserve installed software.".into(),
            "User-installed tools would need to be reinstalled individually.".into(),
            "Must reinstall explicitly".into(),
            Vec::new(),
        );
    }

    // ── Unsafe: home root ──
    if eng.category == Category::UserHomeRoot {
        return (
            "Protect home directory.".into(),
            "Contains all personal files, projects, and configurations.".into(),
            "Not regenerable — user must recreate from scratch".into(),
            Vec::new(),
        );
    }

    // ── Unknown fallback ──
    (
        "Manual review recommended.".into(),
        "Classification uncertain — cannot determine safety automatically.".into(),
        String::new(),
        Vec::new(),
    )
}

/// Estimate rebuild cost time using ecosystem knowledge.
///
/// Returns a human-readable duration estimate based on ecosystem type and
/// typical rebuild speeds observed on modern hardware (2024+).
///
/// v8.6: New — deterministic rebuild cost estimation for execution ordering.
pub(crate) fn estimate_rebuild_cost(
    ecosystem: Option<Ecosystem>,
    size_bytes: u64,
    category: Category,
) -> (String, u8) {
    // u8 score: lower = cheaper (0 = instant, 10 = hours+)

    if size_bytes == 0 {
        return ("Instant".into(), 0);
    }

    let size_mb = size_bytes as f64 / 1_048_576.0;
    let size_gb = size_bytes as f64 / 1_073_741_824.0;

    match category {
        Category::BuildCache | Category::TemporaryFile | Category::GeneratedContent => {
            match ecosystem {
                Some(Ecosystem::Rust) => {
                    if size_gb > 1.0 {
                        (format!("~{:.0}m (cargo build)", size_gb * 2.0), 6)
                    } else if size_mb > 100.0 {
                        (format!("~{:.0}m (cargo build)", size_mb * 0.02), 4)
                    } else {
                        ("<1m (cargo build)".into(), 2)
                    }
                }
                Some(Ecosystem::Node) => {
                    if size_gb > 0.5 {
                        (format!("~{:.0}m (npm install && build)", size_gb * 3.0), 5)
                    } else if size_mb > 100.0 {
                        ("~1-3m (npm install)".into(), 3)
                    } else {
                        ("<1m (npm install)".into(), 2)
                    }
                }
                Some(Ecosystem::Python) => {
                    if size_gb > 0.5 {
                        ("~2-5m (pip install)".into(), 4)
                    } else {
                        ("<1m (pip install)".into(), 2)
                    }
                }
                Some(Ecosystem::Go) => {
                    if size_gb > 0.5 {
                        (format!("~{:.0}m (go build)", size_gb * 1.5), 5)
                    } else {
                        ("<1m (go build)".into(), 2)
                    }
                }
                _ => {
                    if size_gb > 1.0 {
                        (format!("~{:.0}m (rebuild)", size_gb * 5.0), 7)
                    } else if size_mb > 100.0 {
                        (format!("~{:.0}m (rebuild)", size_mb * 0.05), 5)
                    } else {
                        ("<1m (rebuild)".into(), 2)
                    }
                }
            }
        }
        Category::Cache
        | Category::BrowserCache
        | Category::CacheRegistry
        | Category::AIModelCache => ("Instant (auto-regenerated)".into(), 0),
        Category::DependencySource | Category::DownloadedArtifact => {
            if size_gb > 1.0 {
                (format!("~{:.0}m (re-download)", size_gb * 4.0), 6)
            } else {
                (format!("~{:.0}s (re-download)", size_mb * 0.5), 3)
            }
        }
        Category::ToolchainManager | Category::ToolchainInstallation => (
            format!("~{:.0}m (reinstall toolchain)", size_gb.max(1.0) * 10.0),
            8,
        ),
        _ => {
            if size_gb > 1.0 {
                (format!("~{:.0}m", size_gb * 5.0), 7)
            } else {
                (format!("~{:.0}s", size_mb * 2.0), 4)
            }
        }
    }
}
