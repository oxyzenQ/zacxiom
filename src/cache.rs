//! Cache domain classification.
//!
//! Maps file paths to known cache domains using pattern matching.
//! v1 uses static path patterns; v2+ will add heuristic detection.

use crate::rules::CacheDomain;
use std::path::Path;

/// Classify a file path into a cache domain.
pub fn classify(path: &Path) -> CacheDomain {
    let path_str = path.to_string_lossy().to_lowercase();

    // Browser caches
    if path_str.contains("/.cache/mozilla")
        || path_str.contains("/.cache/chromium")
        || path_str.contains("/.cache/google-chrome")
        || path_str.contains("/.cache/brave")
        || path_str.contains("/.cache/edge")
        || path_str.contains("/.mozilla/firefox")
        || path_str.contains("/.config/chromium")
        || path_str.contains("/.config/google-chrome")
    {
        return CacheDomain::Browser;
    }

    // Build artifacts
    if path_str.contains("/target/")
        || path_str.contains("/node_modules/")
        || path_str.contains("/__pycache__/")
        || path_str.contains(".pyc")
        || path_str.contains("/.gradle/")
        || path_str.contains("/build/")
        || path_str.contains("/.next/")
        || path_str.contains("/dist/")
    {
        return CacheDomain::BuildArtifact;
    }

    // Package manager caches
    if path_str.starts_with("/var/cache/apt/")
        || path_str.starts_with("/var/cache/pacman/")
        || path_str.starts_with("/var/cache/yum/")
        || path_str.starts_with("/var/cache/dnf/")
        || path_str.starts_with("/var/cache/zypp/")
    {
        return CacheDomain::PackageManager;
    }

    // Developer caches
    if path_str.contains("/.cargo/registry/")
        || path_str.contains("/.npm/_cacache/")
        || path_str.contains("/.m2/repository/")
        || path_str.contains("/.cache/pip/")
        || path_str.contains("/.cache/yarn/")
    {
        return CacheDomain::Developer;
    }

    // System caches (general /var/cache, /tmp)
    if path_str.starts_with("/var/cache/") {
        return CacheDomain::System;
    }
    if path_str.starts_with("/tmp/") {
        return CacheDomain::System;
    }

    // User cache directories
    if path_str.contains("/.cache/") || path_str.contains("/cache/") {
        return CacheDomain::UserData;
    }

    CacheDomain::Unknown
}

/// Human-readable reason string for a cache domain classification.
#[allow(dead_code)]
pub fn reason_for(domain: &CacheDomain) -> &'static str {
    match domain {
        CacheDomain::Browser => "Browser cache data",
        CacheDomain::System => "System cache directory",
        CacheDomain::BuildArtifact => "Build artifact or dependency cache",
        CacheDomain::PackageManager => "Package manager cache",
        CacheDomain::Developer => "Developer tooling cache",
        CacheDomain::UserData => "User cache data",
        CacheDomain::Unknown => "Unknown cache type",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_cache() {
        assert_eq!(
            classify(Path::new("/home/user/.cache/mozilla/firefox/abc/cache2")),
            CacheDomain::Browser
        );
        assert_eq!(
            classify(Path::new("/home/user/.cache/chromium/Default/Cache/data")),
            CacheDomain::Browser
        );
    }

    #[test]
    fn test_build_artifact() {
        assert_eq!(
            classify(Path::new("/home/user/project/target/debug/build")),
            CacheDomain::BuildArtifact
        );
        assert_eq!(
            classify(Path::new("/home/user/project/node_modules/lodash/index.js")),
            CacheDomain::BuildArtifact
        );
    }

    #[test]
    fn test_package_manager() {
        assert_eq!(
            classify(Path::new("/var/cache/apt/archives/libc6.deb")),
            CacheDomain::PackageManager
        );
    }

    #[test]
    fn test_system_cache() {
        assert_eq!(
            classify(Path::new("/var/cache/man/index.db")),
            CacheDomain::System
        );
        assert_eq!(
            classify(Path::new("/tmp/something.tmp")),
            CacheDomain::System
        );
    }

    #[test]
    fn test_unknown() {
        assert_eq!(
            classify(Path::new("/home/user/Documents/report.pdf")),
            CacheDomain::Unknown
        );
    }
}
