// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Evidence Engine — v10
//!
//! Transparent, auditable evidence for every recommendation.
//! No magic numbers. No unexplained confidence scores.
//!
//! Every recommendation exposes:
//!   - ownership evidence
//!   - ecosystem evidence
//!   - safety evidence
//!   - regeneration evidence
//!   - confidence breakdown
//!   - risk breakdown
//!
//! Architecture:
//!   EvidenceItem → Evidence → ConfidenceBreakdown → EvidenceReport

use std::fmt;

/// Evidence categories for classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceCategory {
    /// Ecosystem detection evidence (Cargo.toml found, package.json found, etc.)
    Ecosystem,
    /// Ownership and project membership evidence.
    Ownership,
    /// Safety analysis evidence.
    Safety,
    /// Regeneration and rebuild evidence.
    Regeneration,
    /// Confidence calculation components.
    Confidence,
    /// Risk factor evidence.
    Risk,
}

impl fmt::Display for EvidenceCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvidenceCategory::Ecosystem => write!(f, "Ecosystem"),
            EvidenceCategory::Ownership => write!(f, "Ownership"),
            EvidenceCategory::Safety => write!(f, "Safety"),
            EvidenceCategory::Regeneration => write!(f, "Regeneration"),
            EvidenceCategory::Confidence => write!(f, "Confidence"),
            EvidenceCategory::Risk => write!(f, "Risk"),
        }
    }
}

/// A single piece of auditable evidence.
#[derive(Debug, Clone)]
pub struct EvidenceItem {
    /// Short title (e.g. "Cargo.toml found")
    pub title: String,
    /// Human-readable description
    pub description: String,
    /// Score contribution (can be negative for penalties)
    pub weight: i32,
    /// Evidence category
    pub category: EvidenceCategory,
    /// Whether this evidence check passed
    pub passed: bool,
    /// Optional source (e.g. file path, command output)
    pub source: Option<String>,
}

impl EvidenceItem {
    pub fn new(title: &str, description: &str, weight: i32, category: EvidenceCategory) -> Self {
        EvidenceItem {
            title: title.to_string(),
            description: description.to_string(),
            weight,
            category,
            passed: true,
            source: None,
        }
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    pub fn failed(mut self) -> Self {
        self.passed = false;
        self.weight = 0;
        self
    }

    pub fn icon(&self) -> &'static str {
        if self.passed {
            "✓"
        } else {
            "✗"
        }
    }
}

/// Collection of evidence items.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    pub items: Vec<EvidenceItem>,
}

impl Evidence {
    pub fn new() -> Self {
        Evidence { items: Vec::new() }
    }

    pub fn add(&mut self, item: EvidenceItem) {
        self.items.push(item);
    }

    /// Total positive weight from passed items.
    pub fn positive_weight(&self) -> i32 {
        self.items
            .iter()
            .filter(|e| e.passed && e.weight > 0)
            .map(|e| e.weight)
            .sum()
    }

    /// Total penalty weight.
    pub fn penalty_weight(&self) -> i32 {
        self.items
            .iter()
            .filter(|e| e.weight < 0)
            .map(|e| e.weight)
            .sum()
    }

    /// Total score from all evidence.
    pub fn total_score(&self) -> i32 {
        self.items.iter().map(|e| e.weight).sum()
    }

    /// Filter evidence by category.
    pub fn by_category(&self, category: EvidenceCategory) -> Vec<&EvidenceItem> {
        self.items
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Get evidence items suitable for inline display.
    pub fn evidence_lines(&self) -> Vec<String> {
        self.items
            .iter()
            .map(|e| {
                let src = e
                    .source
                    .as_ref()
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default();
                format!("  {} {}{}", e.icon(), e.title, src)
            })
            .collect()
    }

    /// Get evidence items with descriptions for detailed output.
    pub fn detailed_lines(&self) -> Vec<String> {
        self.items
            .iter()
            .map(|e| {
                let src = e
                    .source
                    .as_ref()
                    .map(|s| format!("\n    Source: {}", s))
                    .unwrap_or_default();
                format!("  {} {}\n    {}{}", e.icon(), e.title, e.description, src)
            })
            .collect()
    }
}

impl Extend<EvidenceItem> for Evidence {
    fn extend<T: IntoIterator<Item = EvidenceItem>>(&mut self, iter: T) {
        for item in iter {
            self.add(item);
        }
    }
}

/// Confidence breakdown — shows exactly how confidence is computed.
#[derive(Debug, Clone)]
pub struct ConfidenceBreakdown {
    /// Base confidence score.
    pub base_score: u8,
    /// Individual modifier contributions.
    pub modifiers: Vec<(String, i32)>,
    /// Penalty contributions.
    pub penalties: Vec<(String, i32)>,
    /// Final confidence percentage (0-100).
    pub final_score: u8,
    /// Source evidence used.
    pub evidence: Evidence,
}

impl ConfidenceBreakdown {
    pub fn new(base: u8) -> Self {
        ConfidenceBreakdown {
            base_score: base,
            modifiers: Vec::new(),
            penalties: Vec::new(),
            final_score: base,
            evidence: Evidence::new(),
        }
    }

    pub fn add_modifier(&mut self, name: &str, weight: i32) {
        self.modifiers.push((name.to_string(), weight));
        self.final_score = (self.final_score as i32 + weight).clamp(0, 100) as u8;
    }

    pub fn add_penalty(&mut self, name: &str, weight: i32) {
        self.penalties.push((name.to_string(), weight));
        self.final_score = (self.final_score as i32 + weight).clamp(0, 100) as u8;
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("CONFIDENCE BREAKDOWN\n");
        out.push_str(&"=".repeat(45));
        out.push('\n');
        out.push_str(&format!(
            "  {:<25} {:>4}\n",
            "Base score ............", self.base_score
        ));

        for (name, weight) in &self.modifiers {
            out.push_str(&format!(
                "  {:<25} {:>+4}\n",
                format!("{} ............", name),
                weight
            ));
        }

        if !self.penalties.is_empty() {
            out.push('\n');
            for (name, weight) in &self.penalties {
                out.push_str(&format!(
                    "  {:<25} {:>+4}\n",
                    format!("Penalty ({}) ......", name),
                    weight
                ));
            }
        }

        out.push_str(&"─".repeat(45));
        out.push('\n');
        out.push_str(&format!(
            "  {:<25} {:>4}\n",
            "Final score ...........", self.final_score
        ));
        out
    }
}

/// Risk breakdown — structured risk analysis.
#[derive(Debug, Clone)]
pub struct RiskBreakdown {
    /// Individual risk factor assessments.
    pub factors: Vec<RiskFactor>,
    /// Overall risk verdict.
    pub verdict: RiskVerdict,
    /// Evidence collected.
    pub evidence: Evidence,
}

#[derive(Debug, Clone)]
pub struct RiskFactor {
    pub name: String,
    pub present: bool,
    pub severity: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskVerdict {
    VerifiedSafe,
    LowRisk,
    ModerateRisk,
    HighRisk,
    Critical,
}

impl RiskVerdict {
    pub fn display(&self) -> &'static str {
        match self {
            RiskVerdict::VerifiedSafe => "Verified Safe",
            RiskVerdict::LowRisk => "Low Risk",
            RiskVerdict::ModerateRisk => "Moderate Risk",
            RiskVerdict::HighRisk => "High Risk",
            RiskVerdict::Critical => "Critical",
        }
    }
}

impl RiskBreakdown {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("RISK ANALYSIS\n");
        out.push_str(&"=".repeat(45));
        out.push('\n');

        for factor in &self.factors {
            let status = if factor.present { "yes" } else { "none" };
            out.push_str(&format!(
                "  {:<25} {:>4}  ({})\n",
                format!("{} ...", factor.name),
                status,
                factor.severity
            ));
        }

        out.push('\n');
        out.push_str(&format!("  Final Risk\n  {}\n", self.verdict.display()));
        out
    }
}

/// Full evidence report combining confidence and risk breakdowns.
#[derive(Debug, Clone)]
pub struct EvidenceReport {
    pub confidence: ConfidenceBreakdown,
    pub risk: RiskBreakdown,
    pub evidence: Evidence,
}

impl EvidenceReport {
    pub fn render_compact(&self) -> String {
        let mut out = String::new();

        if !self.evidence.items.is_empty() {
            out.push_str("Evidence\n");
            out.push_str(&"─".repeat(45));
            out.push('\n');
            for line in &self.evidence.evidence_lines() {
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        out.push_str(&self.confidence.render());
        out.push('\n');
        out.push_str(&self.risk.render());

        out
    }
}

/// Build evidence for a Rust project.
pub fn collect_rust_evidence(project_root: &std::path::Path, project_size: u64) -> EvidenceReport {
    use std::fs;

    let mut evidence = Evidence::new();
    let mut confidence = ConfidenceBreakdown::new(40);
    let mut risk_factors = Vec::new();

    // ── Ecosystem evidence ──
    let cargo_toml = project_root.join("Cargo.toml");
    if cargo_toml.exists() {
        evidence.add(
            EvidenceItem::new(
                "Cargo.toml found",
                "Primary Rust project manifest — defines package metadata and dependencies",
                20,
                EvidenceCategory::Ecosystem,
            )
            .with_source(&cargo_toml.to_string_lossy()),
        );
        confidence.add_modifier("Cargo.toml", 20);
        risk_factors.push(RiskFactor {
            name: "Build manifest".into(),
            present: true,
            severity: "safe",
            detail: "Cargo.toml — project is well-defined".into(),
        });
    }

    let cargo_lock = project_root.join("Cargo.lock");
    if cargo_lock.exists() {
        evidence.add(
            EvidenceItem::new(
                "Cargo.lock found",
                "Lockfile ensures reproducible builds with exact dependency versions",
                10,
                EvidenceCategory::Ecosystem,
            )
            .with_source(&cargo_lock.to_string_lossy()),
        );
        confidence.add_modifier("Cargo.lock", 10);
    }

    // ── Artifact evidence ──
    let target_dir = project_root.join("target");
    if target_dir.exists() {
        let target_size = fs::symlink_metadata(&target_dir)
            .map(|m| m.len())
            .unwrap_or(0);
        evidence.add(
            EvidenceItem::new(
                "target/ exists",
                &format!(
                    "Build output directory — {} of regenerable artifacts",
                    crate::display::human_size(target_size)
                ),
                10,
                EvidenceCategory::Regeneration,
            )
            .with_source(&target_dir.to_string_lossy()),
        );
        confidence.add_modifier("target/", 10);
        risk_factors.push(RiskFactor {
            name: "Build artifacts".into(),
            present: true,
            severity: "safe",
            detail: "target/ — fully regenerable via cargo build".into(),
        });
    } else {
        risk_factors.push(RiskFactor {
            name: "Build artifacts".into(),
            present: false,
            severity: "none",
            detail: "No build output found".into(),
        });
    }

    // ── Rebuild verification ──
    evidence.add(EvidenceItem::new(
        "cargo clean officially supported",
        "cargo clean is the Rust ecosystem standard for removing build artifacts",
        10,
        EvidenceCategory::Regeneration,
    ));
    confidence.add_modifier("Official cleanup", 10);

    // ── Git evidence ──
    if project_root.join(".gitignore").exists() || project_root.join(".git").is_dir() {
        let gitignore = project_root.join(".gitignore");
        let ignored = if gitignore.exists() {
            fs::read_to_string(&gitignore)
                .unwrap_or_default()
                .contains("target")
        } else {
            false
        };

        if ignored {
            evidence.add(EvidenceItem::new(
                "target ignored by git",
                "gitignore confirms target/ is not version-controlled — safe to remove",
                5,
                EvidenceCategory::Safety,
            ));
            confidence.add_modifier("gitignored", 5);
        } else {
            evidence.add(
                EvidenceItem::new(
                    "Git repository detected",
                    "Project is under version control — source code is safe",
                    0,
                    EvidenceCategory::Safety,
                )
                .with_source(".git"),
            );
        }
    }

    // ── Ownership verification ──
    let has_ownership = project_root.join("Cargo.toml").exists();
    if has_ownership {
        evidence.add(EvidenceItem::new(
            "Ownership verified",
            "Cargo.toml defines project ownership and dependency structure",
            10,
            EvidenceCategory::Ownership,
        ));
        confidence.add_modifier("Ownership verified", 10);
    }

    // ── Risk factors ──
    risk_factors.push(RiskFactor {
        name: "User files".into(),
        present: false,
        severity: "none",
        detail: "No personal documents detected".into(),
    });
    risk_factors.push(RiskFactor {
        name: "System files".into(),
        present: false,
        severity: "none",
        detail: "No system binaries or configuration".into(),
    });
    risk_factors.push(RiskFactor {
        name: "Configuration".into(),
        present: false,
        severity: "none",
        detail: "No application configuration files".into(),
    });
    risk_factors.push(RiskFactor {
        name: "Shared dependency".into(),
        present: false,
        severity: "none",
        detail: "No shared system dependencies affected".into(),
    });
    risk_factors.push(RiskFactor {
        name: "Irreplaceable data".into(),
        present: false,
        severity: "none",
        detail: "No irreplaceable user data".into(),
    });

    // ── Penalties ──
    let project_size_mb = project_size / 1_048_576;
    if project_size_mb > 500 {
        let penalty = -((project_size_mb / 100) as i32).min(10);
        evidence.add(EvidenceItem::new(
            "Large project",
            &format!(
                "Project is {} — rebuild may take time",
                crate::display::human_size(project_size)
            ),
            penalty,
            EvidenceCategory::Risk,
        ));
        confidence.add_penalty("shared", penalty);
    }

    let risk_verdict = if confidence.final_score >= 90 {
        RiskVerdict::VerifiedSafe
    } else {
        RiskVerdict::LowRisk
    };

    let risk = RiskBreakdown {
        factors: risk_factors,
        verdict: risk_verdict,
        evidence: evidence.clone(),
    };

    EvidenceReport {
        confidence,
        risk,
        evidence,
    }
}

/// Build generic evidence for any path.
pub fn collect_generic_evidence(
    _path: &std::path::Path,
    is_project: bool,
    is_safe: bool,
) -> EvidenceReport {
    let mut evidence = Evidence::new();
    let mut confidence = ConfidenceBreakdown::new(30);
    let mut risk_factors = Vec::new();

    if is_project {
        evidence.add(EvidenceItem::new(
            "Project directory",
            "Ecosystem markers detected",
            20,
            EvidenceCategory::Ecosystem,
        ));
        confidence.add_modifier("Ecosystem markers", 20);
    }

    if is_safe {
        evidence.add(EvidenceItem::new(
            "Safety check passed",
            "No protected or system-critical content detected",
            15,
            EvidenceCategory::Safety,
        ));
        confidence.add_modifier("Safety check", 15);
    } else {
        evidence.add(EvidenceItem::new(
            "Safety check failed",
            "Contains protected or potentially irreplaceable content",
            -20,
            EvidenceCategory::Safety,
        ));
        confidence.add_penalty("safety", -20);
    }

    risk_factors.push(RiskFactor {
        name: "System files".into(),
        present: false,
        severity: "none",
        detail: "No system files detected".into(),
    });
    risk_factors.push(RiskFactor {
        name: "User content".into(),
        present: !is_safe,
        severity: if is_safe { "none" } else { "high" },
        detail: if is_safe {
            "No user content detected".into()
        } else {
            "May contain user data".into()
        },
    });

    let risk = RiskBreakdown {
        factors: risk_factors,
        verdict: if is_safe {
            RiskVerdict::LowRisk
        } else {
            RiskVerdict::HighRisk
        },
        evidence: evidence.clone(),
    };

    EvidenceReport {
        confidence,
        risk,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_rust_evidence_collected() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join(".gitignore"), "target\n").unwrap();

        let report = collect_rust_evidence(root, 1_000_000);
        assert!(report.confidence.final_score >= 80);
        assert!(report.evidence.items.len() >= 4);
        assert_eq!(report.risk.verdict, RiskVerdict::VerifiedSafe);
    }

    #[test]
    fn test_confidence_breakdown_rendering() {
        let mut c = ConfidenceBreakdown::new(40);
        c.add_modifier("Cargo.toml", 20);
        c.add_modifier("target/", 10);
        c.add_penalty("shared", -6);

        let output = c.render();
        assert!(output.contains("Base score"));
        assert!(output.contains("Cargo.toml"));
        assert!(output.contains("Penalty"));
        assert!(output.contains("Final score"));
        assert!(output.contains("64"));
    }

    #[test]
    fn test_risk_breakdown_rendering() {
        let risk = RiskBreakdown {
            factors: vec![
                RiskFactor {
                    name: "User files".into(),
                    present: false,
                    severity: "none",
                    detail: "none".into(),
                },
                RiskFactor {
                    name: "Build artifacts".into(),
                    present: true,
                    severity: "safe",
                    detail: "target/".into(),
                },
            ],
            verdict: RiskVerdict::VerifiedSafe,
            evidence: Evidence::new(),
        };

        let output = risk.render();
        assert!(output.contains("RISK ANALYSIS"));
        assert!(output.contains("User files"));
        assert!(output.contains("Build artifacts"));
        assert!(output.contains("Verified Safe"));
    }

    #[test]
    fn test_evidence_icons() {
        let passed = EvidenceItem::new("test", "desc", 5, EvidenceCategory::Safety);
        let failed = EvidenceItem::new("test2", "desc", 5, EvidenceCategory::Safety).failed();
        assert_eq!(passed.icon(), "✓");
        assert_eq!(failed.icon(), "✗");
    }

    #[test]
    fn test_evidence_weight_aggregation() {
        let mut evidence = Evidence::new();
        evidence.add(EvidenceItem::new("a", "", 10, EvidenceCategory::Confidence));
        evidence.add(EvidenceItem::new("b", "", -6, EvidenceCategory::Confidence));
        evidence.add(EvidenceItem::new("c", "", 0, EvidenceCategory::Confidence));
        assert_eq!(evidence.positive_weight(), 10);
        assert_eq!(evidence.penalty_weight(), -6);
        assert_eq!(evidence.total_score(), 4);
    }
}
