// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Engine types — the single source of truth for classification results.
//!
//! v7: Artifact Intelligence taxonomy. Every classified object belongs to
//! one of 12 artifact kinds, each with distinct ownership, regenerability,
//! and deletion impact characteristics.
//!
//! The taxonomy separates "cache" (disposable, auto-regenerated) from
//! "dependency source" (re-downloadable but expensive) from "installed
//! software" (user-managed, not auto-cleanable).

use std::path::PathBuf;

/// What kind of artifact is at this path?
///
/// v7 Artifact Taxonomy — ordered by increasing deletion risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    // ═══════════════════════════════════════════════════════════
    // SYSTEM — Never cleanable. OS-critical infrastructure.
    // ═══════════════════════════════════════════════════════════
    SystemBinary,
    SystemConfiguration,
    SystemData,
    VirtualFilesystem,

    // ═══════════════════════════════════════════════════════════
    // USER-CRITICAL — Never auto-clean. Contains personal data.
    // ═══════════════════════════════════════════════════════════
    UserHomeRoot,
    SecurityCredential,
    UserDocument,
    UserMedia,
    UserDesktop,

    // ═══════════════════════════════════════════════════════════
    // CONFIGURATION — Manual review. Settings customized by user.
    // ═══════════════════════════════════════════════════════════
    ShellConfiguration,
    ApplicationConfiguration,
    EnvironmentFile,

    // ═══════════════════════════════════════════════════════════
    // CACHE — Fully disposable. Auto-regenerated on next use.
    // Deleting has zero functional impact beyond temporary slowdown.
    // ═══════════════════════════════════════════════════════════
    /// Browser cache — rebuilt automatically while browsing.
    BrowserCache,
    /// Generic application cache — apps regenerate when needed.
    Cache,
    /// Package manager download cache (apt, pacman, npm _cacache).
    /// Metadata about what to download, not the downloaded content itself.
    CacheRegistry,
    /// Temporary files — designed to be cleaned.
    TemporaryFile,

    // ═══════════════════════════════════════════════════════════
    // BUILD ARTIFACT — Generated from source. Fully regenerable.
    // Deleting forces a rebuild, but no external download needed.
    // ═══════════════════════════════════════════════════════════
    /// Build output (target/, dist/, build/, .next/).
    /// Generated from source + dependencies via build command.
    BuildCache,
    /// Generated content (documentation, compiled man pages, etc.)
    /// Re-generable from installed toolchain or source.
    GeneratedContent,

    // ═══════════════════════════════════════════════════════════
    // DEPENDENCY SOURCE — Re-downloadable but expensive.
    // Deleting forces network re-download. Offline builds may fail.
    // Requires --smart to clean. Not disposable cache.
    // ═══════════════════════════════════════════════════════════
    /// Downloaded crate/package sources (cargo registry, npm _npx, etc.)
    /// Not cache — this is installed dependency source code.
    DependencySource,
    /// Downloaded SDK or tool artifact (cargo registry .crate files, etc.)
    /// Regenerable via re-download but large and bandwidth-intensive.
    DownloadedArtifact,
    /// Dependency lockfile — ensures reproducible builds.
    /// Auto-regenerable but loss breaks build reproducibility.
    DependencyLockfile,

    // ═══════════════════════════════════════════════════════════
    // INSTALLED SOFTWARE — User-managed tooling.
    // Not cache, not auto-regenerated. Removal requires reinstall.
    // ═══════════════════════════════════════════════════════════
    /// Toolchain manager (rustup, nvm, sdkman).
    /// Manages installed tool versions. Deleting loses version info.
    ToolchainManager,
    /// Toolchain installation (rustup toolchains, nvm node versions).
    /// Installed compiler/runtime. Deleting forces full reinstall.
    ToolchainInstallation,
    /// User-installed software package (cargo install, npm -g).
    /// Not auto-regenerated. Must be explicitly reinstalled.
    InstalledSoftware,

    // ═══════════════════════════════════════════════════════════
    // PROJECT ASSET — User-authored content.
    // Never auto-cleanable. Deleting means permanent data loss.
    // ═══════════════════════════════════════════════════════════
    /// Project workspace — root directory of a software project.
    ProjectWorkspace,
    /// Source code directory — user-authored code.
    SourceDirectory,
    /// Build manifest — project definition (Cargo.toml, package.json).
    BuildManifest,
    /// Project asset — shell scripts, configs that are part of the project.
    ProjectAsset,

    // ═══════════════════════════════════════════════════════════
    // APPLICATION DATA — Review before cleaning.
    // May contain user data, preferences, or state.
    // ═══════════════════════════════════════════════════════════
    ApplicationData,
    DockerStorage,
    GameData,
    AIModelCache,

    // ═══════════════════════════════════════════════════════════
    // FALLBACK
    // ═══════════════════════════════════════════════════════════
    Unknown,
}

impl Category {
    /// Which artifact family does this category belong to?
    /// v7: Maps to the 12-kind artifact taxonomy for tier/confidence.
    pub fn artifact_family(&self) -> ArtifactFamily {
        match self {
            Category::SystemBinary
            | Category::SystemConfiguration
            | Category::SystemData
            | Category::VirtualFilesystem => ArtifactFamily::System,

            Category::UserHomeRoot
            | Category::SecurityCredential
            | Category::UserDocument
            | Category::UserMedia
            | Category::UserDesktop => ArtifactFamily::UserCritical,

            Category::ShellConfiguration
            | Category::ApplicationConfiguration
            | Category::EnvironmentFile => ArtifactFamily::Configuration,

            Category::BrowserCache
            | Category::Cache
            | Category::CacheRegistry
            | Category::TemporaryFile => ArtifactFamily::Cache,

            Category::BuildCache | Category::GeneratedContent => ArtifactFamily::BuildArtifact,

            Category::DependencySource
            | Category::DownloadedArtifact
            | Category::DependencyLockfile => ArtifactFamily::DependencySource,

            Category::ToolchainManager
            | Category::ToolchainInstallation
            | Category::InstalledSoftware => ArtifactFamily::InstalledSoftware,

            Category::ProjectWorkspace
            | Category::SourceDirectory
            | Category::BuildManifest
            | Category::ProjectAsset => ArtifactFamily::ProjectAsset,

            Category::ApplicationData
            | Category::DockerStorage
            | Category::GameData
            | Category::AIModelCache => ArtifactFamily::ApplicationData,

            Category::Unknown => ArtifactFamily::Unknown,
        }
    }

    /// Is this category ever safe to auto-clean in default mode?
    /// v7: Only TRUE cache categories are auto-cleanable.
    pub fn is_cleanable(&self) -> bool {
        matches!(
            self,
            Category::Cache
                | Category::BuildCache
                | Category::BrowserCache
                | Category::CacheRegistry
                | Category::TemporaryFile
                | Category::GeneratedContent
        )
    }

    /// Is this category cleanable with --smart?
    /// v7: Dependency sources and downloaded artifacts require --smart.
    pub fn is_smart_cleanable(&self) -> bool {
        matches!(
            self,
            Category::DependencySource
                | Category::DownloadedArtifact
                | Category::DependencyLockfile
                | Category::ToolchainInstallation
                | Category::ToolchainManager
        )
    }

    /// Is this category NEVER cleanable?
    pub fn is_protected(&self) -> bool {
        matches!(
            self,
            Category::SystemBinary
                | Category::SystemConfiguration
                | Category::SystemData
                | Category::VirtualFilesystem
                | Category::SecurityCredential
                | Category::ProjectWorkspace
                | Category::SourceDirectory
                | Category::BuildManifest
                | Category::ProjectAsset
                | Category::InstalledSoftware
        )
    }

    /// Human-readable display name.
    pub fn display(&self) -> &'static str {
        match self {
            Category::SystemBinary => "System Binary",
            Category::SystemConfiguration => "System Configuration",
            Category::SystemData => "System Data",
            Category::VirtualFilesystem => "Virtual Filesystem",
            Category::UserHomeRoot => "User Home Directory",
            Category::SecurityCredential => "Security Credential",
            Category::UserDocument => "User Document",
            Category::UserMedia => "User Media",
            Category::UserDesktop => "User Desktop",
            Category::ShellConfiguration => "Shell Config File",
            Category::ApplicationConfiguration => "Application Configuration",
            Category::EnvironmentFile => "Environment Config",
            Category::Cache => "Application Cache",
            Category::BuildCache => "Build Output",
            Category::CacheRegistry => "Package Download Cache",
            Category::BrowserCache => "Browser Cache",
            Category::TemporaryFile => "Temporary File",
            Category::GeneratedContent => "Generated Content",
            Category::DependencySource => "Dependency Source",
            Category::DownloadedArtifact => "Downloaded Artifact",
            Category::DependencyLockfile => "Dependency Lockfile",
            Category::ToolchainManager => "Toolchain Manager",
            Category::ToolchainInstallation => "Toolchain Installation",
            Category::InstalledSoftware => "Installed Software",
            Category::ProjectWorkspace => "Project Workspace",
            Category::SourceDirectory => "Source Code Directory",
            Category::BuildManifest => "Package Manifest",
            Category::ProjectAsset => "Project Asset",
            Category::ApplicationData => "Application Data",
            Category::DockerStorage => "Docker Storage",
            Category::GameData => "Game Data",
            Category::AIModelCache => "AI Model Cache",
            Category::Unknown => "Unknown",
        }
    }
}

/// v7: Artifact family — the top-level classification tier.
/// Maps categories to their semantic family for policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactFamily {
    System,
    UserCritical,
    Configuration,
    Cache,
    BuildArtifact,
    DependencySource,
    InstalledSoftware,
    ProjectAsset,
    ApplicationData,
    Unknown,
}

impl ArtifactFamily {
    /// Can this family be auto-cleaned in default (safe) mode?
    pub fn is_auto_cleanable(&self) -> bool {
        matches!(self, ArtifactFamily::Cache | ArtifactFamily::BuildArtifact)
    }

    /// Can this family be cleaned with --smart?
    pub fn is_smart_cleanable(&self) -> bool {
        matches!(
            self,
            ArtifactFamily::DependencySource | ArtifactFamily::InstalledSoftware
        )
    }

    /// Display name for the family.
    pub fn display(&self) -> &'static str {
        match self {
            ArtifactFamily::System => "System Infrastructure",
            ArtifactFamily::UserCritical => "User-Critical Data",
            ArtifactFamily::Configuration => "Configuration",
            ArtifactFamily::Cache => "Disposable Cache",
            ArtifactFamily::BuildArtifact => "Build Output",
            ArtifactFamily::DependencySource => "Dependency Source",
            ArtifactFamily::InstalledSoftware => "Installed Software",
            ArtifactFamily::ProjectAsset => "Project Asset",
            ArtifactFamily::ApplicationData => "Application Data",
            ArtifactFamily::Unknown => "Unclassified",
        }
    }
}

/// How risky is it to delete this?
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Critical, // System binary, SSH key — never delete
    High,     // User document, config — manual review
    Moderate, // Application data — review before clean
    Low,      // Cache that takes time to regenerate
    Minimal,  // Fully regenerable cache — safe to clean
}

impl RiskLevel {
    pub fn display(&self) -> &'static str {
        match self {
            RiskLevel::Critical => "Critical — never delete",
            RiskLevel::High => "High — manual review required",
            RiskLevel::Moderate => "Moderate — review before cleaning",
            RiskLevel::Low => "Low — safe with review",
            RiskLevel::Minimal => "Minimal — safe to clean",
        }
    }
}

/// The result of classifying a single path.
///
/// v7: Enriched with artifact intelligence — ownership, regeneration,
/// dependency, and reasoning information.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub path: PathBuf,
    pub category: Category,
    pub risk_level: RiskLevel,
    /// Confidence in the classification (0.0–1.0). Deprecated: use `confidence`.
    pub confidence: f32,
    /// Confidence score 0-100 (v6.3).
    pub confidence_score: u8,
    /// Human-readable confidence label.
    pub confidence_explanation: String,
    /// Detailed confidence reasons.
    pub confidence_reasons: Vec<String>,
    /// Size in bytes, if known.
    pub size: Option<u64>,
    /// Why this classification was chosen.
    pub reasons: Vec<String>,
    /// Is this regenerable?
    pub regenerable: bool,
    /// The rule name that matched (for debugging).
    pub matched_by: String,

    // ── v7: Artifact Intelligence fields ──────────────────────
    /// Who created this artifact? (e.g. "Cargo", "Rustup", "npm", "Browser")
    pub created_by: String,
    /// How to regenerate this artifact? (e.g. "cargo build", "rustup toolchain install")
    pub regenerated_by: String,
    /// What does this artifact depend on? (e.g. "Cargo.toml", "package.json")
    pub depends_on: String,
    /// What happens if this artifact is deleted?
    pub deletion_impact: String,
    /// Why is this classification correct? (reasoning chain for --deep)
    pub classification_reasoning: Vec<String>,
}

impl ClassificationResult {
    pub fn new(path: PathBuf) -> Self {
        ClassificationResult {
            path,
            category: Category::Unknown,
            risk_level: RiskLevel::Moderate,
            confidence: 0.0,
            confidence_score: 0,
            confidence_explanation: String::new(),
            confidence_reasons: Vec::new(),
            size: None,
            reasons: Vec::new(),
            regenerable: false,
            matched_by: "default".to_string(),
            created_by: String::new(),
            regenerated_by: String::new(),
            depends_on: String::new(),
            deletion_impact: String::new(),
            classification_reasoning: Vec::new(),
        }
    }

    pub fn with_category(mut self, cat: Category) -> Self {
        self.category = cat;
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reasons.push(reason.into());
        self
    }

    pub fn with_risk(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self
    }

    pub fn with_confidence(mut self, conf: f32) -> Self {
        self.confidence = conf.clamp(0.0, 1.0);
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn matched_by(mut self, rule: impl Into<String>) -> Self {
        self.matched_by = rule.into();
        self
    }
}
