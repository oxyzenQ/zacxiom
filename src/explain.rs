// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Explainability engine v4 — pure presentation layer.
//!
//! v6.2.5: Classification logic moved to `zacxiom-engine`.
//! explain.rs is now presentation-only: consumes ClassificationResult, renders cards.
//! No path matching. No if/else chains. No domain detection.

use crate::confidence::{confidence, Tier};
use crate::engine::{Category, ClassificationResult};
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

/// Generate an explanation from classified files and engine results.
pub fn explain_path(path: &str, classified: &[ClassifiedFile]) -> Explanation {
    // Use the engine to classify this path
    let eng_result = crate::engine::classify(std::path::Path::new(path));

    // v6.3.2: Engine category is authoritative for explain tier.
    // Legacy classified files are used ONLY for size/count, not tier.
    let tier = category_to_tier(&eng_result.category);

    let total_size: u64 = classified.iter().map(|f| f.size).sum();

    explain_domain(&eng_result, total_size, tier, classified.len())
}

/// Convert engine Category to confidence Tier.
fn category_to_tier(cat: &Category) -> Tier {
    match cat {
        Category::SystemBinary
        | Category::SystemConfiguration
        | Category::SystemData
        | Category::VirtualFilesystem
        | Category::SecurityCredential => Tier::Protected,

        Category::UserHomeRoot
        | Category::UserDocument
        | Category::UserMedia
        | Category::UserDesktop
        | Category::ShellConfiguration
        | Category::ProjectWorkspace
        | Category::SourceDirectory
        | Category::BuildManifest => Tier::Minimal,

        Category::ApplicationConfiguration
        | Category::EnvironmentFile
        | Category::ApplicationData
        | Category::ProjectAsset
        | Category::ToolchainManager
        | Category::ToolchainInstallation
        | Category::InstalledSoftware => Tier::Moderate,

        Category::Cache
        | Category::CacheRegistry
        | Category::DockerStorage
        | Category::GameData
        | Category::AIModelCache
        | Category::DependencySource
        | Category::DownloadedArtifact
        | Category::DependencyLockfile => Tier::High,

        Category::BuildCache
        | Category::GeneratedContent
        | Category::BrowserCache
        | Category::TemporaryFile => Tier::Maximum,

        Category::Unknown => Tier::Moderate,
    }
}

/// Produce a semantic title that is project-specific based on the matched rule.
/// The generic Category display name (e.g. "Project Workspace") is overridden
/// with a specific name (e.g. "Rust Project Workspace") when the matched rule
/// provides enough context.
fn semantic_title(cat: &Category, matched_by: &str) -> String {
    match (cat, matched_by) {
        (_, "project-rust") => "Rust Project Workspace".into(),
        (_, "project-node") => "Node.js Project Workspace".into(),
        (_, "project-go") => "Go Project Workspace".into(),
        (Category::BuildManifest, "rust-cargo-toml") => "Rust Package Manifest".into(),
        (Category::BuildManifest, "node-package-json") => "Node.js Package Manifest".into(),
        (Category::BuildManifest, "go-mod") => "Go Module File".into(),
        (Category::DependencyLockfile, "rust-cargo-lock") => "Rust Dependency Lockfile".into(),
        (Category::DependencyLockfile, "node-package-lock") => "Node.js Dependency Lockfile".into(),
        (Category::ToolchainManager, _) => "Rust Toolchain Manager".into(),
        (Category::ToolchainInstallation, _) => "Installed Rust Toolchains".into(),
        _ => cat.display().to_string(),
    }
}

/// Generate a domain-level explanation from an engine result.
pub fn explain_domain(
    eng: &ClassificationResult,
    total_size: u64,
    tier: Tier,
    file_count: usize,
) -> Explanation {
    let (what, why, consequence, recommendation) = render_category(&eng.category, eng);
    let title = semantic_title(&eng.category, &eng.matched_by);

    Explanation {
        title,
        what: what.to_string(),
        size: simulator::human_size(total_size),
        tier,
        why_safe: why.to_string(),
        consequence: consequence.to_string(),
        recommendation: recommendation
            .unwrap_or_else(|| default_recommendation(&tier))
            .to_string(),
        file_count: Some(file_count),
    }
}

fn render_category(
    cat: &Category,
    eng: &ClassificationResult,
) -> (&'static str, String, String, Option<String>) {
    let reasons = eng.reasons.join("; ");

    match cat {
        Category::SystemBinary => (
            "Installed application executable or binary.",
            format!("This is an installed program, not cache or data. Never deletable. {}", reasons),
            "The application would stop working. May require reinstallation.".into(),
            Some("Never delete. This is installed software.".into()),
        ),
        Category::SystemConfiguration => (
            "System-wide configuration file (located in /etc or equivalent).",
            format!("These files control system behavior. Never auto-clean. {}", reasons),
            "System services or environment may break. Could prevent boot or login.".into(),
            Some("Never delete system configuration files.".into()),
        ),
        Category::SystemData => (
            "System library, shared resource, or boot data.",
            format!("Part of the operating system or installed packages. {}", reasons),
            "Applications or the system itself may fail.".into(),
            Some("Never delete system libraries or data.".into()),
        ),
        Category::VirtualFilesystem => (
            "Virtual kernel filesystem (proc/sys/dev). Not real files.",
            "These are kernel interfaces, not files on disk. They use zero actual storage. Never touch.".into(),
            "System instability, kernel panics, or device malfunction.".into(),
            Some("Never interact with virtual filesystems.".into()),
        ),
        Category::UserHomeRoot => (
            "Your home directory — contains all personal files, projects, configs, downloads, and caches.",
            "The home directory contains a mix of important files AND cache. Zacxiom scans subdirectories individually — never clean the entire home directory.".into(),
            "Personal files, projects, and configuration would be permanently lost.".into(),
            Some("Never clean entire home directory. Use `zacxiom scan` to find specific cache locations.".into()),
        ),
        Category::SecurityCredential => (
            "Security credential, key, or identity file.",
            format!("These are cryptographic identities. Never auto-clean. {}", reasons),
            "Deleting keys permanently removes access to systems or encrypted data. Cannot be regenerated.".into(),
            Some("Never delete without understanding consequences.".into()),
        ),
        Category::UserDocument => (
            "Personal documents and downloaded files.",
            "These are your personal files — may contain irreplaceable content. Zacxiom does NOT auto-clean user content.".into(),
            "Personal documents would be permanently deleted. Not recoverable from cache or cloud.".into(),
            Some("Never auto-cleaned. Review each file before deleting.".into()),
        ),
        Category::UserMedia => (
            "Personal media files — music, pictures, videos.",
            "Your media library. Not cache. Review before deleting anything here.".into(),
            "Media files permanently deleted.".into(),
            Some("Never auto-cleaned. Manual review only.".into()),
        ),
        Category::UserDesktop => (
            "Desktop files — your primary workspace.",
            "Contains files you intentionally placed on your desktop. Not cache.".into(),
            "Desktop files permanently deleted.".into(),
            Some("Never auto-cleaned. Review manually.".into()),
        ),
        Category::ShellConfiguration => (
            "Shell configuration file — defines your terminal environment, aliases, and PATH.",
            "This is a configuration file, not cache. Contains your personal shell customizations.".into(),
            "Deleting resets shell environment to defaults. Custom aliases, PATH, and prompt settings are lost.".into(),
            Some("Do not auto-delete. Review manually before removing.".into()),
        ),
        Category::ApplicationConfiguration => (
            "Application configuration file — settings and preferences.",
            "Contains customized settings. Most apps recreate defaults if deleted, but customizations are lost.".into(),
            "Apps reset to factory defaults. Custom settings and preferences are lost.".into(),
            Some("Review before deleting. Settings will be lost.".into()),
        ),
        Category::EnvironmentFile => (
            "Environment variables file — may contain secrets and API keys.",
            "These files define environment variables for applications. May contain sensitive values.".into(),
            "Environment variables reset, applications may fail to configure correctly.".into(),
            Some("Review carefully before deleting.".into()),
        ),
        Category::Cache => (
            "User application cache data — temporary files stored by desktop and CLI applications.",
            format!("Applications rebuild their cache automatically. Safe to remove. {}", reasons),
            "Applications may take slightly longer to start or reload content until caches rebuild.".into(),
            None,
        ),
        Category::BuildCache => (
            "Build tool cache — compiled artifacts, dependency downloads.",
            format!("Build tools regenerate these automatically. Safe to remove. {}", reasons),
            "Next build may take longer while artifacts are regenerated.".into(),
            None,
        ),
        Category::CacheRegistry => (
            "Package manager download cache.",
            format!("Package managers re-download from their registries. {}", reasons),
            "Next install/update may take longer while packages re-download.".into(),
            None,
        ),
        Category::DependencySource => (
            "Dependency source archive — downloaded package source code.",
            "Not runtime cache. This is downloaded source code used by build tools. Regenerable via re-download, but large and bandwidth-intensive.".into(),
            "Builds may fail until dependencies are re-downloaded. Offline builds will break.".into(),
            Some("Safe to reclaim if disk space is critical and you have internet access.".into()),
        ),
        Category::GeneratedContent => (
            "Generated content — documentation, compiled metadata, or derived files.",
            "Automatically generated from installed toolchains or source code. Fully regenerable.".into(),
            "Content will be regenerated on next build or tool invocation.".into(),
            None,
        ),
        Category::InstalledSoftware => (
            "User-installed software — manually installed tools and packages.",
            "Not auto-regenerated. Must be explicitly reinstalled if removed.".into(),
            "Software will be uninstalled and must be reinstalled manually.".into(),
            Some("Only remove if you no longer need this software.".into()),
        ),
        Category::ProjectAsset => (
            "Project asset — shell scripts, build configs, and other project files.",
            "Part of the project workspace. Not regenerable — user-authored content.".into(),
            "Project may break if required scripts or configs are missing.".into(),
            Some("Never auto-remove. Review carefully before deleting.".into()),
        ),
        Category::BrowserCache => (
            "Browser cache, temporary internet files, and service worker storage.",
            "Browsers rebuild their cache automatically as you browse. No bookmarks, passwords, or settings are affected.".into(),
            "Websites may load slightly slower on first visit until the cache rebuilds.".into(),
            None,
        ),
        Category::TemporaryFile => (
            "Temporary file — designed to be cleaned.",
            "These files were created for temporary use. Safe to remove.".into(),
            "No impact. These files were intended to be temporary.".into(),
            None,
        ),
        Category::DownloadedArtifact => (
            "Downloaded software component or SDK artifact.",
            "Can be redownloaded from the internet, but large and time-consuming. Not cache — this is installed software that can be restored.".into(),
            "Next build or tool invocation will redownload the component. This may take significant time and bandwidth.".into(),
            Some("Safe to reclaim if disk space is critical. Otherwise keep — redownloading is expensive.".into()),
        ),
        Category::ApplicationData => (
            "User application data — saved states, databases, user-generated content.",
            "This is where applications store your actual data. Review file-by-file before deleting.".into(),
            "Application data may be permanently lost. Some apps sync to cloud, others do not.".into(),
            Some("Manual review required before cleaning.".into()),
        ),
        Category::DockerStorage => (
            "Docker image layers, build cache, and container storage.",
            "Docker rebuilds images from Dockerfiles. Running containers are NOT affected.".into(),
            "Next `docker build` will rebuild layers from cache or Dockerfile.".into(),
            None,
        ),
        Category::GameData => (
            "Game compatibility data and shader caches.",
            "Steam and Proton regenerate these when launching games. Game saves are separate.".into(),
            "Games may take longer to launch first time. Game saves should be unaffected.".into(),
            None,
        ),
        Category::AIModelCache => (
            "Downloaded AI/ML model files (HuggingFace, Ollama, Torch, etc.).",
            "Models can be re-downloaded from their sources. Training checkpoints are permanently deleted — review carefully.".into(),
            "Models will re-download when needed. Checkpoints are lost permanently.".into(),
            Some("Review checkpoints carefully. Models can be re-downloaded.".into()),
        ),
        Category::ProjectWorkspace => (
            "Project workspace containing source code, manifests, and build configuration.",
            format!("This is a project root directory — source code and configuration live here. Not cache. Never auto-clean. {}", reasons),
            "Project source code and configuration are lost. The project would need to be restored from version control or rebuilt.".into(),
            Some("Never auto-clean. Version control protects against accidental loss, but uncommitted changes would be lost.".into()),
        ),
        Category::SourceDirectory => (
            "Source code directory — contains the project's implementation files.",
            "This is where source code lives. Not cache, not build output. Deleting loses source code.".into(),
            "Source code files are permanently deleted. Only recoverable from version control if committed.".into(),
            Some("Never auto-clean. Source code is never safe to delete.".into()),
        ),
        Category::BuildManifest => (
            "Package manifest file — defines project identity, dependencies, and build configuration.",
            format!("This is the project's primary definition file. Without it, the project cannot be built or identified. {}", reasons),
            "The project loses its build definition. Dependencies, scripts, and metadata are lost. Must be recreated manually.".into(),
            Some("Never delete. This file defines the project.".into()),
        ),
        Category::DependencyLockfile => (
            "Dependency lockfile — pins exact dependency versions for reproducible builds.",
            format!("Lockfiles ensure every build uses the same dependency versions. Regenerable from the manifest, but team reproducibility depends on committed lockfiles. {}", reasons),
            "Dependency versions may change on next build. Builds may break or produce different outputs until lockfile is regenerated.".into(),
            Some("Regenerable but important. Do not delete in shared projects without team coordination.".into()),
        ),
        Category::ToolchainManager => (
            "Toolchain manager directory — manages installed compiler and tool versions.",
            "Contains toolchain installations and update metadata. Not cache — this is installed development tooling. Regenerable but requires significant time and bandwidth to restore.".into(),
            "All installed toolchains and update state are removed. Development tools like rustup will need to reinstall everything (potentially gigabytes).".into(),
            Some("Not recommended for auto-clean. Use --smart to reclaim. Reinstalling is expensive.".into()),
        ),
        Category::ToolchainInstallation => (
            "Installed compiler toolchain — the actual compiler, standard library, and development tools.",
            "Can be reinstalled by the toolchain manager, but this requires significant download time and bandwidth. This is NOT build cache — it is installed development tooling. Deleting it means your compiler disappears.".into(),
            "The compiler and tools are removed. All builds will fail until the toolchain is reinstalled. Reinstall may take several minutes and hundreds of MB to several GB.".into(),
            Some("Not recommended for auto-clean. Use --smart to reclaim. Otherwise keep — reinstalling is expensive.".into()),
        ),
        Category::Unknown => (
            "Storage that may be safe to clean after review.",
            format!("No strong risk signals detected, but verify before deleting. {}", reasons),
            "Verify the specific files before proceeding.".into(),
            Some("Review manually before cleaning.".into()),
        ),
    }
}

fn default_recommendation(tier: &Tier) -> String {
    match tier {
        Tier::Maximum => "Safe to reclaim if disk space needed.".into(),
        Tier::High => "Safe with review. Use `zacxiom clean --smart`.".into(),
        Tier::Moderate => "Review recommended. Use `zacxiom clean --force` after review.".into(),
        Tier::Low | Tier::Minimal => "Manual review required. Do not auto-clean.".into(),
        Tier::Protected => "Will never be cleaned automatically by Zacxiom.".into(),
    }
}

/// Explain a single file (fallback when no domain match).
pub fn explain_file(file: &ClassifiedFile) -> Explanation {
    let tier = confidence(file);
    let eng = crate::engine::classify(std::path::Path::new(&file.path));
    explain_domain(&eng, file.size, tier, 1)
}

/// Render an explanation card with confidence (v6.3).
pub fn render_card(exp: &Explanation, eng: Option<&crate::engine::ClassificationResult>) -> String {
    let mut out = String::new();
    let stars = exp.tier.stars();

    out.push_str(&format!("\n{}  {}\n", stars, exp.title));
    out.push_str(&format!("{}\n", "─".repeat(60)));
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

    // v6.3: Confidence score
    if let Some(eng) = eng {
        out.push_str(&format!(
            "\n  Confidence: {}%  —  {}\n",
            eng.confidence_score, eng.confidence_explanation
        ));
        for reason in &eng.confidence_reasons {
            out.push_str(&format!("    {}\n", reason));
        }
    }
    out
}
