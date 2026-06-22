// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cache domain classification — v6.1.0: expanded developer + gaming + AI coverage.
//!
//! Maps file paths to known cache domains using pattern matching.
//! Order matters: specific patterns (e.g. /.cargo/registry/) must
//! match before general patterns (e.g. /.cache/).

use crate::rules::CacheDomain;
use std::path::Path;

/// Classify a file path into a cache domain.
pub fn classify(path: &Path) -> CacheDomain {
    let path_str = path.to_string_lossy().to_lowercase();

    // ── Browser caches ──────────────────────────────────────────
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

    // ── Build artifacts ─────────────────────────────────────────
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

    // ── Package manager caches ──────────────────────────────────
    if path_str.starts_with("/var/cache/apt/")
        || path_str.starts_with("/var/cache/pacman/")
        || path_str.starts_with("/var/cache/yum/")
        || path_str.starts_with("/var/cache/dnf/")
        || path_str.starts_with("/var/cache/zypp/")
    {
        return CacheDomain::PackageManager;
    }

    // ── Developer caches ────────────────────────────────────────
    if path_str.contains("/.cargo/registry/")
        || path_str.contains("/.cargo/git/")
        || path_str.contains("/.rustup/toolchains/")
        || path_str.contains("/.npm/_cacache/")
        || path_str.contains("/.cache/pip/")
        || path_str.contains("/.cache/uv/")
        || path_str.contains("/.m2/repository/")
        || path_str.contains("/.cache/yarn/")
        || path_str.contains("/.cache/pnpm/")
    {
        return CacheDomain::Developer;
    }

    // ── Docker / container caches ────────────────────────────────
    if path_str.contains("/.docker/overlay2")
        || path_str.contains("/.docker/buildkit")
        || path_str.contains("/.docker/containers")
        || path_str.starts_with("/var/lib/docker/overlay2")
        || path_str.starts_with("/var/lib/docker/buildkit")
        || path_str.contains("/.local/share/containers/")
    {
        return CacheDomain::Developer;
    }

    // ── AI / ML caches ──────────────────────────────────────────
    if path_str.contains("/.cache/huggingface/")
        || path_str.contains("/.cache/torch/")
        || path_str.contains("/.cache/ollama/")
        || path_str.contains("/.cache/modelscope/")
    {
        return CacheDomain::Developer;
    }

    // ── Gaming caches ───────────────────────────────────────────
    if path_str.contains("/.steam/steam/steamapps/shadercache")
        || path_str.contains("/.steam/steam/steamapps/downloading")
    {
        return CacheDomain::System;
    }
    if path_str.contains("/.steam/steam/steamapps/compatdata")
        || path_str.contains("/.local/share/steam/steamapps/compatdata")
    {
        return CacheDomain::UserData;
    }
    if path_str.contains("/.cache/dxvk-cache")
        || path_str.contains("/.cache/vkd3d")
        || path_str.contains("/.cache/mesa")
        || path_str.contains("/.cache/mesa_shader_cache")
    {
        return CacheDomain::System;
    }
    if path_str.contains("/.local/share/lutris") || path_str.contains("/.config/heroic") {
        return CacheDomain::UserData;
    }

    // ── Desktop / Trash ─────────────────────────────────────────
    if path_str.contains("/.local/share/trash/") {
        return CacheDomain::UserData;
    }

    // ── Flatpak caches ─────────────────────────────────────────
    if path_str.contains("/.var/app/") && path_str.contains("/cache/") {
        return CacheDomain::UserData;
    }

    // ── Snap caches ────────────────────────────────────────────
    if path_str.contains("/snap/") && path_str.contains("/.cache/") {
        return CacheDomain::UserData;
    }

    // ── AUR helper caches (Arch Linux) ─────────────────────────
    if path_str.contains("/.cache/yay/") || path_str.contains("/.cache/paru/") {
        return CacheDomain::PackageManager;
    }

    // ── System caches ───────────────────────────────────────────
    if path_str.starts_with("/var/cache/") {
        return CacheDomain::System;
    }
    if path_str.starts_with("/tmp/") {
        return CacheDomain::System;
    }

    // ── General user cache directories ──────────────────────────
    if path_str.contains("/.cache/") {
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

    // ── v6.1.0 expanded coverage tests ──────────────────────────

    #[test]
    fn test_rustup_toolchains() {
        assert_eq!(
            classify(Path::new(
                "/home/dev/.rustup/toolchains/stable-x86_64/lib/rustlib/src"
            )),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_cargo_git_checkouts() {
        assert_eq!(
            classify(Path::new("/home/dev/.cargo/git/db/some-crate-hash")),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_docker_overlay() {
        assert_eq!(
            classify(Path::new("/home/dev/.docker/overlay2/hash/diff")),
            CacheDomain::Developer
        );
        assert_eq!(
            classify(Path::new("/var/lib/docker/overlay2/hash/diff")),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_podman_storage() {
        assert_eq!(
            classify(Path::new(
                "/home/dev/.local/share/containers/storage/overlay/hash"
            )),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_ai_caches() {
        assert_eq!(
            classify(Path::new("/home/dev/.cache/huggingface/hub/models--bert")),
            CacheDomain::Developer
        );
        assert_eq!(
            classify(Path::new("/home/dev/.cache/ollama/models/blobs/sha256")),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_uv_cache() {
        assert_eq!(
            classify(Path::new("/home/dev/.cache/uv/wheels-v0/hash")),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_pnpm_cache() {
        assert_eq!(
            classify(Path::new("/home/dev/.cache/pnpm/store/v3/files/hash")),
            CacheDomain::Developer
        );
    }

    #[test]
    fn test_steam_shader_cache() {
        assert_eq!(
            classify(Path::new(
                "/home/gamer/.steam/steam/steamapps/shadercache/440/fozmediav1"
            )),
            CacheDomain::System
        );
    }

    #[test]
    fn test_steam_compatdata_proton() {
        assert_eq!(
            classify(Path::new(
                "/home/gamer/.steam/steam/steamapps/compatdata/440/pfx/drive_c"
            )),
            CacheDomain::UserData
        );
    }

    #[test]
    fn test_dxvk_vkd3d_cache() {
        assert_eq!(
            classify(Path::new("/home/gamer/.cache/dxvk-cache/AppId.dxvk-cache")),
            CacheDomain::System
        );
        assert_eq!(
            classify(Path::new("/home/gamer/.cache/vkd3d/some-cache")),
            CacheDomain::System
        );
    }

    #[test]
    fn test_lutris_heroic() {
        assert_eq!(
            classify(Path::new(
                "/home/gamer/.local/share/lutris/runners/wine/version"
            )),
            CacheDomain::UserData
        );
    }

    #[test]
    fn test_desktop_trash() {
        assert_eq!(
            classify(Path::new("/home/user/.local/share/Trash/files/old.txt")),
            CacheDomain::UserData
        );
    }

    #[test]
    fn test_flatpak_cache() {
        assert_eq!(
            classify(Path::new(
                "/home/user/.var/app/org.mozilla.firefox/cache/mozilla/firefox/cache2/entry"
            )),
            CacheDomain::UserData
        );
    }

    #[test]
    fn test_snap_cache() {
        // Snap Firefox cache contains both /snap/ AND /.cache/mozilla/
        // Browser check runs first — correctly identifies it as browser data
        assert_eq!(
            classify(Path::new(
                "/home/user/snap/firefox/common/.cache/mozilla/firefox/cache2/entry"
            )),
            CacheDomain::Browser
        );
    }

    #[test]
    fn test_aur_helper_caches() {
        assert_eq!(
            classify(Path::new("/home/user/.cache/yay/firefox")),
            CacheDomain::PackageManager
        );
        assert_eq!(
            classify(Path::new("/home/user/.cache/paru/clone/firefox")),
            CacheDomain::PackageManager
        );
    }

    #[test]
    fn test_steam_downloading() {
        assert_eq!(
            classify(Path::new(
                "/home/gamer/.steam/steam/steamapps/downloading/440"
            )),
            CacheDomain::System
        );
    }

    #[test]
    fn test_classification_order_maintains_priority() {
        // Cargo registry is Developer, not UserData (despite containing /.cache/ in .cargo)
        assert_eq!(
            classify(Path::new(
                "/home/dev/snap/cache-test/.cargo/registry/cache/crate.crate"
            )),
            CacheDomain::Developer
        );
    }
}
