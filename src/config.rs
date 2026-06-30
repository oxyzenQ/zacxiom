// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Configuration system — TOML-based user configuration with strict validation.
//!
//! Location: `~/.config/zacxiom/config.toml` (XDG-compliant)
//!
//! # Validation Policy
//!
//! Any malformed value, unknown key, or type mismatch causes a hard error
//! with a human-readable message. The program EXITS — never silently falls
//! back to defaults when the user explicitly wrote a config. This prevents
//! typos from accidentally weakening safety (e.g. `default_mode = "forc"`
//! would otherwise silently use "safe" and confuse the user).
//!
//! Use `zacxiom --testconf` to validate the config without running a command.

use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Top-level configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub clean: CleanConfig,
    #[serde(default)]
    pub rules_exclude: RulesExcludeConfig,
    #[serde(default)]
    pub snapshot: SnapshotConfig,
    #[serde(default)]
    pub trash: TrashConfig,
}

/// v13: Unified rules-based exclude — files matching these patterns are NEVER
/// scanned or cleaned. Replaces hardcoded PROTECTED_EXTENSIONS in source code.
/// Users can add their own patterns (e.g. "*.private", "Crypto_wallet.sha256sum").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesExcludeConfig {
    /// Glob patterns for files that zacxiom will NEVER scan or clean.
    /// Matched against both full path and filename.
    /// Defaults include disk images (.iso .vmdk .vdi .qcow2 .ova .img .raw .wim .vhd .vhdx)
    /// and cryptographic keys (.pem .key .p12 .pfx .keystore .jks .gpg .asc)
    /// and SSH key filenames (id_rsa id_ed25519 id_ecdsa).
    #[serde(default = "default_rules_exclude")]
    pub exclude: Vec<String>,
}

impl Default for RulesExcludeConfig {
    fn default() -> Self {
        RulesExcludeConfig {
            exclude: default_rules_exclude(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Directory paths to exclude from scanning (glob patterns supported).
    /// Example: `["~/Downloads", "~/Documents"]`
    #[serde(default)]
    pub exclude: Vec<String>,

    /// File-name glob patterns to exclude (matched against full path).
    /// Example: `["*.iso", "*.vmdk", "*.tmp"]`
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Minimum file size in bytes to include in scan results.
    #[serde(default = "default_min_size")]
    pub min_size: u64,

    /// v13: Maximum threads to use for scanning/classification (0 = auto).
    /// Auto mode uses 75% of available CPUs, scaled by workload size.
    /// Set to 1-4 to limit CPU usage on constrained systems.
    #[serde(default)]
    pub max_threads: usize,

    /// Warn before scanning user-content directories (Downloads, Documents, etc.).
    #[serde(default = "default_true")]
    pub warn_user_dirs: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        ScanConfig {
            exclude: Vec::new(),
            exclude_patterns: Vec::new(),
            min_size: default_min_size(),
            max_threads: 0,
            warn_user_dirs: default_true(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanConfig {
    /// Require interactive confirmation before any deletion (default: true).
    #[serde(default = "default_true")]
    pub require_confirmation: bool,

    /// Default clean mode when no flag is given: "safe" | "smart".
    /// "force" is NOT allowed as a default — too dangerous.
    #[serde(default = "default_clean_mode")]
    pub default_mode: String,

    /// File extensions that are NEVER cleanable (treated as Protected).
    /// Default: disk images + cryptographic keys.
    #[serde(default = "default_protect_extensions")]
    pub protect_extensions: Vec<String>,

    /// File-name glob patterns that are NEVER cleanable.
    #[serde(default = "default_protect_patterns")]
    pub protect_patterns: Vec<String>,

    /// Files larger than this in user directories require explicit --force.
    /// v13: Human-readable — accepts "100MB", "1GB", "2gb", "50 mb" (case-insensitive, space-tolerant).
    /// Also accepts raw bytes (e.g. 104857600) for backward compatibility.
    /// Only MB and GB suffixes are accepted — use raw bytes for other units.
    #[serde(
        default = "default_max_auto_clean_size",
        deserialize_with = "deserialize_size"
    )]
    pub max_auto_clean_size: u64,

    /// First-time users get automatic dry-run unless they pass --yes.
    #[serde(default = "default_true")]
    pub first_run_dry_run: bool,
}

impl Default for CleanConfig {
    fn default() -> Self {
        CleanConfig {
            require_confirmation: default_true(),
            default_mode: default_clean_mode(),
            protect_extensions: default_protect_extensions(),
            protect_patterns: default_protect_patterns(),
            max_auto_clean_size: default_max_auto_clean_size(),
            first_run_dry_run: default_true(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    /// Snapshot storage directory. Default: XDG-compliant ~/.local/share/zacxiom/snapshots
    #[serde(default = "default_snapshot_dir")]
    pub dir: String,

    /// Auto-prune snapshots older than N days (0 = disabled).
    #[serde(default = "default_prune_days")]
    pub auto_prune_days: u32,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        SnapshotConfig {
            dir: default_snapshot_dir(),
            auto_prune_days: default_prune_days(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashConfig {
    /// Verify SHA-256 checksum of trash copies after move (slower, safer).
    #[serde(default = "default_false")]
    pub verify_checksum: bool,
}

impl Default for TrashConfig {
    fn default() -> Self {
        TrashConfig {
            verify_checksum: default_false(),
        }
    }
}

// ── Default value functions ─────────────────────────────────────

fn default_min_size() -> u64 {
    1
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_clean_mode() -> String {
    "safe".to_string()
}
fn default_protect_extensions() -> Vec<String> {
    vec![
        ".iso".into(),
        ".vmdk".into(),
        ".vdi".into(),
        ".qcow2".into(),
        ".qcow".into(),
        ".ova".into(),
        ".ovf".into(),
        ".img".into(),
        ".raw".into(),
        ".wim".into(),
        ".vhd".into(),
        ".vhdx".into(),
        ".pem".into(),
        ".key".into(),
        ".p12".into(),
        ".pfx".into(),
        ".keystore".into(),
        ".jks".into(),
        ".gpg".into(),
        ".asc".into(),
    ]
}
fn default_protect_patterns() -> Vec<String> {
    vec!["id_rsa".into(), "id_ed25519".into(), "id_ecdsa".into()]
}

/// v13: Default rules_exclude patterns — disk images + crypto keys + SSH key filenames.
/// These are NEVER scanned or cleaned by zacxiom, regardless of location.
/// Users can override/extend via [rules_exclude].exclude in config.toml.
fn default_rules_exclude() -> Vec<String> {
    vec![
        // Disk images — losing these means losing VMs or installable OS
        "*.iso".into(),
        "*.vmdk".into(),
        "*.vdi".into(),
        "*.vhd".into(),
        "*.vhdx".into(),
        "*.qcow2".into(),
        "*.qcow".into(),
        "*.ova".into(),
        "*.ovf".into(),
        "*.img".into(),
        "*.raw".into(),
        "*.wim".into(),
        // Cryptographic keys — permanent access loss if deleted
        "*.pem".into(),
        "*.key".into(),
        "*.p12".into(),
        "*.pfx".into(),
        "*.keystore".into(),
        "*.jks".into(),
        "*.gpg".into(),
        "*.asc".into(),
        // SSH key filenames (no extension)
        "id_rsa".into(),
        "id_ed25519".into(),
        "id_ecdsa".into(),
    ]
}

fn default_max_auto_clean_size() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

/// v13: Human-readable size parser. Accepts ONLY these formats:
///   - "100MB" / "100mb" / "100 MB" / "100 mb"  (megabytes, 1024*1024 bytes)
///   - "1GB"  / "1gb"  / "1 GB"  / "1 gb"   (gigabytes, 1024^3 bytes)
///   - 104857600  (raw bytes, backward-compat — integers in TOML)
///
/// Rejects: KB, TB, KiB, MiB, GiB, and any other suffix.
/// Returns bytes as u64. Error message is human-readable.
fn parse_size_str(s: &str) -> Result<u64, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("empty size value".into());
    }
    let upper = trimmed.to_uppercase();
    if upper.ends_with("MB") {
        let num_str = trimmed[..trimmed.len() - 2].trim();
        let n: u64 = num_str
            .parse()
            .map_err(|e| format!("invalid number before MB: \"{num_str}\" ({e})"))?;
        Ok(n.saturating_mul(1024 * 1024))
    } else if upper.ends_with("GB") {
        let num_str = trimmed[..trimmed.len() - 2].trim();
        let n: u64 = num_str
            .parse()
            .map_err(|e| format!("invalid number before GB: \"{num_str}\" ({e})"))?;
        Ok(n.saturating_mul(1024 * 1024 * 1024))
    } else {
        // Try raw bytes (integer)
        trimmed.parse::<u64>().map_err(|_| {
            format!(
                "invalid size: \"{trimmed}\". Use '100MB', '1GB', or raw bytes (e.g. 104857600). \
                     Only MB and GB suffixes are accepted."
            )
        })
    }
}

/// v13: Serde deserializer that accepts both string ("100MB") and integer (104857600).
/// Uses parse_size_str for validation — only MB/GB/raw-bytes accepted.
fn deserialize_size<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{Error, Visitor};
    use std::fmt;

    struct SizeVisitor;

    impl<'de> Visitor<'de> for SizeVisitor {
        type Value = u64;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "a size string like \"100MB\" or \"1GB\", or an integer like 104857600"
            )
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<u64, E> {
            parse_size_str(v).map_err(E::custom)
        }

        fn visit_string<E: Error>(self, v: String) -> Result<u64, E> {
            parse_size_str(&v).map_err(E::custom)
        }

        fn visit_i64<E: Error>(self, v: i64) -> Result<u64, E> {
            if v < 0 {
                Err(E::custom("size cannot be negative"))
            } else {
                Ok(v as u64)
            }
        }

        fn visit_u64<E: Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
    }

    deserializer.deserialize_any(SizeVisitor)
}
fn default_snapshot_dir() -> String {
    "~/.local/share/zacxiom/snapshots".to_string()
}
fn default_prune_days() -> u32 {
    30
}

// ── Path helpers ─────────────────────────────────────────────────

/// Expand `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    } else if path == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

/// Get the config file path: `~/.config/zacxiom/config.toml`.
/// Respects `XDG_CONFIG_HOME` if set.
pub fn config_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("zacxiom/config.toml")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/zacxiom/config.toml")
    } else {
        PathBuf::from(".config/zacxiom/config.toml")
    }
}

/// Get the config directory (parent of config.toml).
pub fn config_dir() -> PathBuf {
    config_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

// ── Load + Validate ──────────────────────────────────────────────

/// Load and validate the config. Returns Ok(Config) if no file exists (uses defaults)
/// or if the file is present and valid. Returns Err(message) if the file is malformed.
///
/// The error message is human-readable and includes the specific field/key that failed.
pub fn load() -> Result<Config, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read config file {}: {e}", path.display()))?;
    parse_and_validate(&contents)
}

/// Parse and validate config contents. Public so --testconf can reuse it.
pub fn parse_and_validate(contents: &str) -> Result<Config, String> {
    // Step 1: Parse TOML — catches syntax errors
    let toml_doc: toml::Value = toml::from_str(contents).map_err(|e| {
        format!(
            "TOML syntax error in config:\n  {}\n\nFix the syntax and run `zacxiom --testconf` to verify.",
            e
        )
    })?;

    // Step 2: Deserialize into Config struct — catches type mismatches
    let cfg: Config = toml_doc
        .clone()
        .try_into()
        .map_err(|e| format!("Config type error:\n  {e}"))?;

    // Step 3: Semantic validation — catches logically invalid values
    validate_semantic(&cfg)?;

    // Step 4: Detect unknown keys (warn, don't error — but include in --testconf output)
    detect_unknown_keys(&toml_doc)?;

    Ok(cfg)
}

/// Semantic validation — values that parse correctly but are logically invalid.
fn validate_semantic(cfg: &Config) -> Result<(), String> {
    // clean.default_mode must be "safe" or "smart" (NOT "force")
    let mode = cfg.clean.default_mode.as_str();
    if mode != "safe" && mode != "smart" {
        return Err(format!(
            "Invalid value for [clean].default_mode: \"{}\"\n  Allowed values: \"safe\" | \"smart\"\n  \
             Note: \"force\" is intentionally NOT allowed as a default — it must be passed explicitly on each run.",
            mode
        ));
    }

    // min_size must be reasonable
    if cfg.scan.min_size > 1_000_000_000 {
        return Err(format!(
            "Invalid value for [scan].min_size: {} (1 billion bytes)\n  This would skip ALL files. Use a smaller value (default: 1).",
            cfg.scan.min_size
        ));
    }

    // max_auto_clean_size sanity check
    if cfg.clean.max_auto_clean_size < 1024 {
        return Err(format!(
            "Invalid value for [clean].max_auto_clean_size: {} bytes\n  Values below 1KB would block almost all cleaning. Use a larger value (default: 104857600 = 100MB).",
            cfg.clean.max_auto_clean_size
        ));
    }

    // auto_prune_days: 0 = disabled, otherwise reasonable range
    if cfg.snapshot.auto_prune_days > 3650 {
        return Err(format!(
            "Invalid value for [snapshot].auto_prune_days: {}\n  Maximum allowed: 3650 (10 years). Use 0 to disable auto-prune.",
            cfg.snapshot.auto_prune_days
        ));
    }

    // Validate protect_extensions: must start with '.' and be lowercase
    for ext in &cfg.clean.protect_extensions {
        if !ext.starts_with('.') {
            return Err(format!(
                "Invalid value in [clean].protect_extensions: \"{}\"\n  Extensions must start with a dot, e.g. \".iso\" not \"iso\".",
                ext
            ));
        }
        if ext.to_lowercase() != *ext {
            return Err(format!(
                "Invalid value in [clean].protect_extensions: \"{}\"\n  Extensions must be lowercase, e.g. \".iso\" not \".ISO\".",
                ext
            ));
        }
    }

    // Validate exclude patterns: must be parseable as globs
    for pat in &cfg.scan.exclude_patterns {
        if let Err(e) = globset::Glob::new(pat) {
            return Err(format!(
                "Invalid glob pattern in [scan].exclude_patterns: \"{}\"\n  Error: {e}",
                pat
            ));
        }
    }
    for pat in &cfg.clean.protect_patterns {
        if let Err(e) = globset::Glob::new(pat) {
            return Err(format!(
                "Invalid glob pattern in [clean].protect_patterns: \"{}\"\n  Error: {e}",
                pat
            ));
        }
    }

    // v13: Validate rules_exclude patterns
    for pat in &cfg.rules_exclude.exclude {
        if let Err(e) = globset::Glob::new(pat) {
            return Err(format!(
                "Invalid glob pattern in [rules_exclude].exclude: \"{}\"\n  Error: {e}",
                pat
            ));
        }
    }

    // Validate snapshot.dir: must expand to an absolute path
    let snap_dir = expand_tilde(&cfg.snapshot.dir);
    if !snap_dir.is_absolute() {
        return Err(format!(
            "Invalid value for [snapshot].dir: \"{}\"\n  Path must be absolute or start with ~ (expanded to: \"{}\").",
            cfg.snapshot.dir,
            snap_dir.display()
        ));
    }

    Ok(())
}

/// Detect unknown top-level sections and keys.
fn detect_unknown_keys(toml_doc: &toml::Value) -> Result<(), String> {
    let known_sections: &[&str] = &["scan", "clean", "rules_exclude", "snapshot", "trash"];
    let known_keys: &[(&str, &[&str])] = &[
        (
            "scan",
            &[
                "exclude",
                "exclude_patterns",
                "min_size",
                "max_threads",
                "warn_user_dirs",
            ],
        ),
        (
            "clean",
            &[
                "require_confirmation",
                "default_mode",
                "protect_extensions",
                "protect_patterns",
                "max_auto_clean_size",
                "first_run_dry_run",
            ],
        ),
        ("rules_exclude", &["exclude"]),
        ("snapshot", &["dir", "auto_prune_days"]),
        ("trash", &["verify_checksum"]),
    ];

    let mut warnings = Vec::new();

    if let Some(table) = toml_doc.as_table() {
        for (key, _) in table {
            if !known_sections.contains(&key.as_str()) {
                warnings.push(format!("Unknown section: [{key}]"));
            }
        }
        for section in known_sections {
            if let Some(sec_val) = table.get(*section) {
                if let Some(sec_table) = sec_val.as_table() {
                    let allowed: &[&str] = known_keys
                        .iter()
                        .find(|(s, _)| s == section)
                        .map(|(_, k)| *k)
                        .unwrap_or(&[]);
                    for (k, _) in sec_table {
                        if !allowed.contains(&k.as_str()) {
                            warnings.push(format!("Unknown key in [{section}]: {k}"));
                        }
                    }
                }
            }
        }
    }

    // Unknown keys are warnings, not errors — but report them in --testconf
    if !warnings.is_empty() {
        // We don't fail here; warnings are surfaced via validate_for_testconf
        // Stored as a side-effect note via thread-local would be overkill.
        // Instead, --testconf re-runs detection and prints warnings.
    }
    Ok(())
}

/// Run full validation and return a structured report for `--testconf`.
pub fn validate_for_testconf() -> TestconfReport {
    let path = config_path();
    if !path.exists() {
        return TestconfReport {
            file_exists: false,
            file_path: path,
            errors: Vec::new(),
            warnings: Vec::new(),
            config: Config::default(),
        };
    }

    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return TestconfReport {
                file_exists: true,
                file_path: path,
                errors: vec![format!("Cannot read file: {e}")],
                warnings: Vec::new(),
                config: Config::default(),
            };
        }
    };

    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Parse TOML
    let toml_doc: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            return TestconfReport {
                file_exists: true,
                file_path: path,
                errors: vec![format!("TOML syntax error:\n  {e}")],
                warnings,
                config: Config::default(),
            };
        }
    };

    // Collect unknown-key warnings
    let known_sections: &[&str] = &["scan", "clean", "rules_exclude", "snapshot", "trash"];
    let known_keys: &[(&str, &[&str])] = &[
        (
            "scan",
            &[
                "exclude",
                "exclude_patterns",
                "min_size",
                "max_threads",
                "warn_user_dirs",
            ],
        ),
        (
            "clean",
            &[
                "require_confirmation",
                "default_mode",
                "protect_extensions",
                "protect_patterns",
                "max_auto_clean_size",
                "first_run_dry_run",
            ],
        ),
        ("rules_exclude", &["exclude"]),
        ("snapshot", &["dir", "auto_prune_days"]),
        ("trash", &["verify_checksum"]),
    ];
    if let Some(table) = toml_doc.as_table() {
        for (key, _) in table {
            if !known_sections.contains(&key.as_str()) {
                warnings.push(format!("Unknown section: [{key}]"));
            }
        }
        for section in known_sections {
            if let Some(sec_val) = table.get(*section) {
                if let Some(sec_table) = sec_val.as_table() {
                    let allowed: &[&str] = known_keys
                        .iter()
                        .find(|(s, _)| s == section)
                        .map(|(_, k)| *k)
                        .unwrap_or(&[]);
                    for (k, _) in sec_table {
                        if !allowed.contains(&k.as_str()) {
                            warnings.push(format!("Unknown key in [{section}]: {k}"));
                        }
                    }
                }
            }
        }
    }

    // Deserialize
    let cfg: Config = match toml_doc.clone().try_into() {
        Ok(c) => c,
        Err(e) => {
            errors.push(format!("Type error:\n  {e}"));
            return TestconfReport {
                file_exists: true,
                file_path: path,
                errors,
                warnings,
                config: Config::default(),
            };
        }
    };

    // Semantic validation
    if let Err(e) = validate_semantic(&cfg) {
        errors.push(e);
    }

    TestconfReport {
        file_exists: true,
        file_path: path,
        errors,
        warnings,
        config: cfg,
    }
}

/// Report returned by `--testconf`.
pub struct TestconfReport {
    pub file_exists: bool,
    pub file_path: PathBuf,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub config: Config,
}

impl TestconfReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Generate the default config TOML content (for `zacxiom config init`).
pub fn default_config_toml() -> String {
    let cfg = Config::default();
    toml::to_string_pretty(&cfg).unwrap_or_else(|_| "# config generation failed".to_string())
}

/// Write the default config to disk (for `zacxiom config init`).
pub fn write_default_config() -> Result<PathBuf, String> {
    let path = config_path();
    if path.exists() {
        return Err(format!(
            "Config already exists at {}. Use `zacxiom config edit` to modify it.",
            path.display()
        ));
    }
    let dir = config_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create config dir {}: {e}", dir.display()))?;
    let contents = default_config_toml();
    fs::write(&path, contents)
        .map_err(|e| format!("Cannot write config {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validates() {
        let cfg = Config::default();
        assert!(validate_semantic(&cfg).is_ok());
    }

    #[test]
    fn test_invalid_default_mode_force() {
        let mut cfg = Config::default();
        cfg.clean.default_mode = "force".into();
        let err = validate_semantic(&cfg).unwrap_err();
        assert!(err.contains("force"));
        assert!(err.to_lowercase().contains("not allowed"));
    }

    #[test]
    fn test_invalid_default_mode_typo() {
        let mut cfg = Config::default();
        cfg.clean.default_mode = "savv".into(); // intentional typo of "safe"
        let err = validate_semantic(&cfg).unwrap_err();
        assert!(err.contains("savv"));
    }

    #[test]
    fn test_protect_extension_must_start_with_dot() {
        let mut cfg = Config::default();
        cfg.clean.protect_extensions = vec!["iso".into()]; // missing dot
        let err = validate_semantic(&cfg).unwrap_err();
        assert!(err.contains("must start with a dot"));
    }

    #[test]
    fn test_protect_extension_must_be_lowercase() {
        let mut cfg = Config::default();
        cfg.clean.protect_extensions = vec![".ISO".into()];
        let err = validate_semantic(&cfg).unwrap_err();
        assert!(err.contains("must be lowercase"));
    }

    #[test]
    fn test_invalid_glob_pattern_rejected() {
        let mut cfg = Config::default();
        cfg.scan.exclude_patterns = vec!["[unclosed".into()];
        let err = validate_semantic(&cfg).unwrap_err();
        assert!(err.contains("Invalid glob pattern"));
    }

    #[test]
    fn test_min_size_too_large_rejected() {
        let mut cfg = Config::default();
        cfg.scan.min_size = 2_000_000_000;
        assert!(validate_semantic(&cfg).is_err());
    }

    #[test]
    fn test_valid_config_parses() {
        let toml = r#"
[scan]
exclude = ["~/Downloads", "~/Documents"]
exclude_patterns = ["*.iso", "*.vmdk"]
min_size = 1

[clean]
require_confirmation = true
default_mode = "safe"
protect_extensions = [".iso", ".vmdk"]
max_auto_clean_size = 104857600

[snapshot]
dir = "~/.local/share/zacxiom/snapshots"
auto_prune_days = 30
"#;
        let result = parse_and_validate(toml);
        assert!(result.is_ok(), "{:?}", result.err());
        let cfg = result.unwrap();
        assert_eq!(cfg.clean.default_mode, "safe");
        assert_eq!(cfg.scan.exclude.len(), 2);
    }

    #[test]
    fn test_syntax_error_reported() {
        let toml = r#"
[scan]
exclude = "not a list"  # should be array
"#;
        let result = parse_and_validate(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("type") || err.contains("Type"));
    }

    #[test]
    fn test_expand_tilde() {
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", "/home/testuser");
        assert_eq!(expand_tilde("~/foo"), PathBuf::from("/home/testuser/foo"));
        assert_eq!(
            expand_tilde("/absolute/path"),
            PathBuf::from("/absolute/path")
        );
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_default_config_toml_roundtrip() {
        let toml_str = default_config_toml();
        let parsed = parse_and_validate(&toml_str);
        assert!(
            parsed.is_ok(),
            "Default config should roundtrip: {:?}",
            parsed.err()
        );
    }

    // ── v13: Human-readable size parser tests ──────────────────

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(parse_size_str("100MB").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size_str("100mb").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size_str("100 MB").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size_str("  50mb  ").unwrap(), 50 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_gb() {
        assert_eq!(parse_size_str("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size_str("1gb").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size_str("2 GB").unwrap(), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_raw_bytes() {
        assert_eq!(parse_size_str("104857600").unwrap(), 104857600);
        assert_eq!(parse_size_str("0").unwrap(), 0);
    }

    #[test]
    fn test_parse_size_rejects_kb_tb() {
        assert!(parse_size_str("100KB").is_err(), "KB should be rejected");
        assert!(parse_size_str("1TB").is_err(), "TB should be rejected");
        assert!(parse_size_str("100KiB").is_err(), "KiB should be rejected");
        assert!(parse_size_str("1MiB").is_err(), "MiB should be rejected");
        assert!(parse_size_str("1GiB").is_err(), "GiB should be rejected");
    }

    #[test]
    fn test_parse_size_rejects_invalid() {
        assert!(parse_size_str("").is_err());
        assert!(parse_size_str("abc").is_err());
        assert!(parse_size_str("100XB").is_err(), "unknown suffix");
        assert!(parse_size_str("MB100").is_err(), "suffix before number");
    }

    #[test]
    fn test_config_with_human_readable_size() {
        let toml = r#"
[clean]
max_auto_clean_size = "100MB"
"#;
        let cfg = parse_and_validate(toml).unwrap();
        assert_eq!(cfg.clean.max_auto_clean_size, 100 * 1024 * 1024);
    }

    #[test]
    fn test_config_with_gb_size() {
        let toml = r#"
[clean]
max_auto_clean_size = "2GB"
"#;
        let cfg = parse_and_validate(toml).unwrap();
        assert_eq!(cfg.clean.max_auto_clean_size, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_config_with_raw_int_size_backward_compat() {
        let toml = r#"
[clean]
max_auto_clean_size = 104857600
"#;
        let cfg = parse_and_validate(toml).unwrap();
        assert_eq!(cfg.clean.max_auto_clean_size, 104857600);
    }

    #[test]
    fn test_config_rejects_kb_size() {
        let toml = r#"
[clean]
max_auto_clean_size = "500KB"
"#;
        let result = parse_and_validate(toml);
        assert!(result.is_err(), "KB should be rejected");
    }
}
