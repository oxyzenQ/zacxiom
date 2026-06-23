// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Engine types — the single source of truth for classification results.
//!
//! Every path through the classifier produces exactly one `ClassificationResult`.
//! No hardcoded strings. No scattered enums. One type system.

use std::path::PathBuf;

/// What kind of thing is at this path?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    // ── System: never cleanable ─────────────────────────────
    SystemBinary,
    SystemConfiguration,
    SystemData,
    VirtualFilesystem,

    // ── User-critical: never auto-clean ─────────────────────
    UserHomeRoot,
    SecurityCredential,
    UserDocument,
    UserMedia,
    UserDesktop,

    // ── Configuration: manual review ────────────────────────
    ShellConfiguration,
    ApplicationConfiguration,
    EnvironmentFile,

    // ── Cache: the primary target ───────────────────────────
    Cache,
    BuildCache,
    PackageCache,
    BrowserCache,
    TemporaryFile,

    // ── Downloaded artifacts: regenerable but expensive ────
    DownloadedArtifact,

    // ── Developer workspace: project structure ────────────
    ProjectWorkspace,
    SourceDirectory,
    BuildManifest,
    DependencyLockfile,
    ShellScript,
    ToolchainManager,
    ToolchainInstallation,

    // ── Application data: review before cleaning ────────────
    ApplicationData,
    DockerStorage,
    GameData,
    AIModelCache,

    // ── Fallback ───────────────────────────────────────────
    Unknown,
}

impl Category {
    /// Is this category ever safe to auto-clean?
    pub fn is_cleanable(&self) -> bool {
        matches!(
            self,
            Category::Cache
                | Category::BuildCache
                | Category::PackageCache
                | Category::BrowserCache
                | Category::TemporaryFile
                | Category::DownloadedArtifact
                | Category::DependencyLockfile
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
            Category::Cache => "User Cache",
            Category::BuildCache => "Build Cache",
            Category::PackageCache => "Package Cache",
            Category::BrowserCache => "Browser Cache",
            Category::TemporaryFile => "Temporary File",
            Category::DownloadedArtifact => "Downloaded Artifact",
            Category::ApplicationData => "Application Data",
            Category::DockerStorage => "Docker Storage",
            Category::GameData => "Game Data",
            Category::AIModelCache => "AI Model Cache",
            Category::ProjectWorkspace => "Project Workspace",
            Category::SourceDirectory => "Source Code Directory",
            Category::BuildManifest => "Package Manifest",
            Category::DependencyLockfile => "Dependency Lockfile",
            Category::ShellScript => "Shell Script",
            Category::ToolchainManager => "Toolchain Manager",
            Category::ToolchainInstallation => "Toolchain Installation",
            Category::Unknown => "Unknown",
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
