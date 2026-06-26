// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ecosystem Knowledge Base — v8.8
//!
//! Centralized repository of ecosystem intelligence.
//! No duplicated rules. No scattered knowledge.
//!
//! Each ecosystem defines: project detection, regenerable artifacts,
//! rebuild commands, cleanup commands, and human-readable explanations.
//!
//! Architecture:
//!   Ecosystem → provides metadata for discovery, regeneration, and classification.

use std::path::Path;

/// Supported ecosystems.
///
/// v8.8: Expanded from 4 to 15 ecosystems covering major build systems,
/// package managers, and development platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Ecosystem {
    Rust,
    Node,
    Python,
    Go,
    Java,
    DotNet,
    Zig,
    CMake,
    Meson,
    Gradle,
    Maven,
    Flutter,
    Android,
    Unity,
    Unreal,
}

impl Ecosystem {
    /// Human-readable display name.
    pub fn display(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "Rust",
            Ecosystem::Node => "Node.js",
            Ecosystem::Python => "Python",
            Ecosystem::Go => "Go",
            Ecosystem::Java => "Java",
            Ecosystem::DotNet => ".NET",
            Ecosystem::Zig => "Zig",
            Ecosystem::CMake => "CMake",
            Ecosystem::Meson => "Meson",
            Ecosystem::Gradle => "Gradle",
            Ecosystem::Maven => "Maven",
            Ecosystem::Flutter => "Flutter",
            Ecosystem::Android => "Android",
            Ecosystem::Unity => "Unity",
            Ecosystem::Unreal => "Unreal Engine",
        }
    }

    /// Project marker files that identify this ecosystem.
    /// Order matters — first match wins during discovery.
    pub fn markers(&self) -> &'static [&'static str] {
        match self {
            Ecosystem::Rust => &["Cargo.toml", "Cargo.lock"],
            Ecosystem::Node => &[
                "package.json",
                "package-lock.json",
                "yarn.lock",
                "pnpm-lock.yaml",
            ],
            Ecosystem::Python => &[
                "pyproject.toml",
                "setup.py",
                "setup.cfg",
                "Pipfile",
                "requirements.txt",
            ],
            Ecosystem::Go => &["go.mod", "go.sum"],
            Ecosystem::Java => &[
                "pom.xml",
                "build.gradle.kts",
                "build.gradle",
                "settings.gradle",
            ],
            Ecosystem::DotNet => &["*.csproj", "*.sln", "*.fsproj", "global.json"],
            Ecosystem::Zig => &["build.zig", "build.zig.zon"],
            Ecosystem::CMake => &["CMakeLists.txt"],
            Ecosystem::Meson => &["meson.build"],
            Ecosystem::Gradle => &[
                "settings.gradle.kts",
                "build.gradle.kts",
                "gradle.properties",
            ],
            Ecosystem::Maven => &["pom.xml"],
            Ecosystem::Flutter => &["pubspec.yaml"],
            Ecosystem::Android => &["AndroidManifest.xml", "build.gradle", "local.properties"],
            Ecosystem::Unity => &[
                "Assets/ProjectSettings/ProjectVersion.txt",
                "Packages/manifest.json",
            ],
            Ecosystem::Unreal => &["*.uproject", "*.uplugin"],
        }
    }

    /// Directories containing regenerable build artifacts.
    pub fn artifact_dirs(&self) -> &'static [&'static str] {
        match self {
            Ecosystem::Rust => &["target"],
            Ecosystem::Node => &["node_modules", "dist", ".next", ".nuxt", "coverage"],
            Ecosystem::Python => &[
                "__pycache__",
                ".venv",
                "venv",
                "dist",
                ".tox",
                ".eggs",
                "*.egg-info",
            ],
            Ecosystem::Go => &["vendor"],
            Ecosystem::Java => &["target", "build", "out", ".gradle", "bin"],
            Ecosystem::DotNet => &["bin", "obj", "packages", ".vs"],
            Ecosystem::Zig => &["zig-out", "zig-cache", ".zig-cache"],
            Ecosystem::CMake => &["build", "cmake-build-*"],
            Ecosystem::Meson => &["builddir", "build"],
            Ecosystem::Gradle => &["build", ".gradle", "app/build"],
            Ecosystem::Maven => &["target"],
            Ecosystem::Flutter => &["build", ".dart_tool", ".flutter-plugins-dependencies"],
            Ecosystem::Android => &[
                "build",
                ".gradle",
                "app/build",
                "captures",
                ".externalNativeBuild",
            ],
            Ecosystem::Unity => &["Library", "Temp", "Obj", "Builds", "Logs"],
            Ecosystem::Unreal => &[
                "Intermediate",
                "DerivedDataCache",
                "Saved",
                "Binaries",
                "Build",
            ],
        }
    }

    /// Rebuild command for regenerating artifacts.
    pub fn rebuild_command(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "cargo build",
            Ecosystem::Node => "npm install && npm run build",
            Ecosystem::Python => "pip install -r requirements.txt",
            Ecosystem::Go => "go build ./...",
            Ecosystem::Java => "mvn compile  # or gradle build",
            Ecosystem::DotNet => "dotnet build",
            Ecosystem::Zig => "zig build",
            Ecosystem::CMake => "cmake --build build",
            Ecosystem::Meson => "meson compile -C builddir",
            Ecosystem::Gradle => "gradle build",
            Ecosystem::Maven => "mvn compile",
            Ecosystem::Flutter => "flutter build",
            Ecosystem::Android => "./gradlew assembleDebug",
            Ecosystem::Unity => {
                "Unity -batchmode -quit -projectPath . -executeMethod Builder.Build"
            }
            Ecosystem::Unreal => "RunUAT BuildCookRun",
        }
    }

    /// Preferred cleanup command (ecosystem-aware, never raw rm -rf).
    pub fn cleanup_command(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "cargo clean",
            Ecosystem::Node => "rm -rf node_modules && npm cache clean --force",
            Ecosystem::Python => "rm -rf __pycache__ .venv venv && pip cache purge",
            Ecosystem::Go => "go clean -cache -modcache",
            Ecosystem::Java => "mvn clean  # or gradle clean",
            Ecosystem::DotNet => "dotnet clean",
            Ecosystem::Zig => "rm -rf zig-out zig-cache",
            Ecosystem::CMake => "rm -rf build cmake-build-*",
            Ecosystem::Meson => "rm -rf builddir",
            Ecosystem::Gradle => "gradle clean",
            Ecosystem::Maven => "mvn clean",
            Ecosystem::Flutter => "flutter clean",
            Ecosystem::Android => "./gradlew clean",
            Ecosystem::Unity => "rm -rf Library Temp Obj Builds Logs",
            Ecosystem::Unreal => "rm -rf Intermediate DerivedDataCache Saved Binaries Build",
        }
    }

    /// Explanation of what the artifacts are and why they're safe to clean.
    pub fn explanation(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "Compiled Rust binaries and dependencies. Fully regenerable from source via cargo build. Official tool: cargo clean.",
            Ecosystem::Node => "JavaScript dependencies and build output. Declared in package.json. Re-downloadable from npm registry. Safe to remove.",
            Ecosystem::Python => "Python bytecode cache and virtual environments. Bytecode compiles automatically. Virtual envs are reproducible from requirements files.",
            Ecosystem::Go => "Vendored Go dependencies and build cache. Dependencies declared in go.mod. Build cache is safe to clear.",
            Ecosystem::Java => "Compiled Java class files and build output. Fully regenerable from source. Maven/Gradle manage dependencies automatically.",
            Ecosystem::DotNet => "Compiled .NET assemblies and intermediate objects. Fully regenerable via dotnet build. NuGet packages are re-downloadable.",
            Ecosystem::Zig => "Compiled Zig binaries and build cache. Fully regenerable via zig build. Build cache is safe to clear.",
            Ecosystem::CMake => "CMake build output and generated files. Fully regenerable from CMakeLists.txt. Re-run cmake to regenerate.",
            Ecosystem::Meson => "Meson build output. Fully regenerable from meson.build. Re-run meson setup to regenerate.",
            Ecosystem::Gradle => "Gradle build output and downloaded dependencies. Fully regenerable. Dependencies from declared repositories.",
            Ecosystem::Maven => "Maven build output. Fully regenerable. Dependencies downloaded from Maven Central or configured repositories.",
            Ecosystem::Flutter => "Flutter build output and tool cache. Fully regenerable via flutter build. Packages declared in pubspec.yaml.",
            Ecosystem::Android => "Android build output, intermediate files. Fully regenerable via Gradle. Dependencies declared in build.gradle.",
            Ecosystem::Unity => "Unity intermediate files, build cache, logs. Unity regenerates Library/ on project open. Builds/ and Logs/ are safe to remove.",
            Ecosystem::Unreal => "Unreal Engine intermediate build files, derived data cache. Engine regenerates these during build. Saves memory-intensive shader compilation.",
        }
    }

    /// Attempt to detect ecosystem from a directory by checking marker files.
    ///
    /// Checks markers in priority order. First match wins.
    pub fn detect(path: &Path) -> Option<Ecosystem> {
        // Check each ecosystem's markers
        for eco in Ecosystem::all() {
            for marker in eco.markers() {
                if marker.contains('*') {
                    // Glob pattern — check by extension
                    let pattern = marker.trim_start_matches("*.");
                    if let Ok(entries) = std::fs::read_dir(path) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.ends_with(&format!(".{}", pattern)) {
                                return Some(eco);
                            }
                        }
                    }
                } else if marker.contains('/') {
                    // Nested path
                    let full = path.join(marker);
                    if full.exists() {
                        return Some(eco);
                    }
                } else {
                    // Simple file
                    if path.join(marker).exists() {
                        return Some(eco);
                    }
                }
            }
        }
        None
    }

    /// All ecosystems in priority order for detection.
    pub fn all() -> [Ecosystem; 15] {
        [
            Ecosystem::Rust,
            Ecosystem::Node,
            Ecosystem::Python,
            Ecosystem::Go,
            Ecosystem::Java,
            Ecosystem::DotNet,
            Ecosystem::Zig,
            Ecosystem::CMake,
            Ecosystem::Meson,
            Ecosystem::Gradle,
            Ecosystem::Maven,
            Ecosystem::Flutter,
            Ecosystem::Android,
            Ecosystem::Unity,
            Ecosystem::Unreal,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(Ecosystem::detect(dir.path()), Some(Ecosystem::Rust));
    }

    #[test]
    fn test_detect_node() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "").unwrap();
        assert_eq!(Ecosystem::detect(dir.path()), Some(Ecosystem::Node));
    }

    #[test]
    fn test_empty_dir_no_detect() {
        let dir = TempDir::new().unwrap();
        assert_eq!(Ecosystem::detect(dir.path()), None);
    }

    #[test]
    fn test_all_ecosystems_have_markers() {
        for eco in Ecosystem::all() {
            assert!(
                !eco.markers().is_empty(),
                "{} has no markers",
                eco.display()
            );
            assert!(
                !eco.artifact_dirs().is_empty(),
                "{} has no artifact dirs",
                eco.display()
            );
            assert!(
                !eco.rebuild_command().is_empty(),
                "{} has no rebuild command",
                eco.display()
            );
            assert!(
                !eco.cleanup_command().is_empty(),
                "{} has no cleanup command",
                eco.display()
            );
            assert!(
                !eco.explanation().is_empty(),
                "{} has no explanation",
                eco.display()
            );
        }
    }

    #[test]
    fn test_detect_flutter() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pubspec.yaml"), "").unwrap();
        assert_eq!(Ecosystem::detect(dir.path()), Some(Ecosystem::Flutter));
    }

    #[test]
    fn test_detect_cmake() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("CMakeLists.txt"), "").unwrap();
        assert_eq!(Ecosystem::detect(dir.path()), Some(Ecosystem::CMake));
    }

    #[test]
    fn test_detect_zig() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.zig"), "").unwrap();
        assert_eq!(Ecosystem::detect(dir.path()), Some(Ecosystem::Zig));
    }
}
