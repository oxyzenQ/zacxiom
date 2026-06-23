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

    let tier = if classified.is_empty() {
        category_to_tier(&eng_result.category)
    } else {
        classified
            .iter()
            .map(confidence)
            .max()
            .unwrap_or(category_to_tier(&eng_result.category))
    };

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
        | Category::ShellConfiguration => Tier::Minimal,

        Category::ApplicationConfiguration
        | Category::EnvironmentFile
        | Category::ApplicationData => Tier::Moderate,

        Category::Cache | Category::DockerStorage | Category::GameData | Category::AIModelCache => {
            Tier::High
        }

        Category::BuildCache
        | Category::PackageCache
        | Category::BrowserCache
        | Category::TemporaryFile => Tier::Maximum,

        Category::Unknown => Tier::Moderate,
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

    Explanation {
        title: eng.category.display().to_string(),
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
        Category::PackageCache => (
            "Package manager download cache.",
            format!("Package managers re-download from their registries. {}", reasons),
            "Next install/update may take longer while packages re-download.".into(),
            None,
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
