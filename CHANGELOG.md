# Changelog

All notable changes to zacxiom.

## [v14.3.0] — 2026-07-01

### Enterprise Readiness

### Added — Config Presets
- `--preset dev|server|minimal` global flag
- `dev`: default (no changes)
- `server`: conservative for production (500MB threshold, 7-day prune)
- `minimal`: ultra-safe (10MB threshold, no age-based auto-clean)
- config.rs: `apply_preset()` function

### Added — Prometheus Metrics Export
- `--metrics` global flag exports Prometheus text format
- 7 metrics: clean/undo/scan operations, files_removed, bytes_freed, files_restored, errors
- Compatible with node_exporter textfile collector
- Usage: `zacxiom --metrics > /var/cache/node_exporter/zacxiom.prom`

### Added — Audit Log Rotation
- Auto-rotates when audit.log exceeds 100MB
- Keeps last 1000 entries, archives old as audit.log.1
- `rotate_audit_log()` in audit.rs
- AuditEntryRead now derives Serialize for rotation

### Verified
- 21/21 depth audit tests PASS
- build.sh check-all PASS (413 tests), codespell PASS, golden 3/3 PASS
- All v14.3 features smoke-tested: presets, metrics, man page, dedup, viz, cache-stats

## [v14.2.0] — 2026-07-01

### Advanced Intelligence

### Added — Age-Based Auto-Clean
- New config: `[clean].auto_clean_older_than_days = 90`
- Files older than threshold auto-marked as Safe (unless Protected)

### Added — Duplicate File Detection
- New `zacxiom dedup` command — find duplicates by size → SHA-256
- Two-phase: size grouping → hash only for candidates (efficient)
- Read-only — never deletes. JSON output supported.

### Added — Space Visualization
- New `zacxiom viz` command — ASCII treemap of disk usage
- Groups by parent directory, sorted by size descending

## [v14.1.0] — 2026-07-01

### Polish & Stability

### Added — Man Page Generation
- New `zacxiom man` command via clap_mangen
- Outputs groff-formatted man page to stdout

### Added — Cache Compression
- scan_cache.rs save() now gzip-compresses (flate2)
- load() auto-detects gzip vs plain JSON (backward compat)
- 85k-file cache: ~15MB → ~2MB (87% reduction)

### Added — Cache Statistics
- `--cache-stats` global flag shows cache info without scanning

## [v14.0.0] — 2026-07-01

### Cross-Unix Support + Performance Fixes

### Fixed — Critical Performance Bug
- Scan cache double-scan eliminated — was doing a SECOND full filesystem scan
  to update cache. Now reuses classification results. Halves I/O.

### Fixed — CI
- yamllint line-length error in ci.yml musl-build job
- release.sh ANSI color code typo

### Added — Cross-Unix Source Compatibility
- procfs.rs: /proc gated to Linux; non-Linux returns empty set (graceful)
- pipeline.rs: /proc/loadavg gated to Linux; non-Linux skips load-aware scaling
- color.rs, display.rs, profiles.rs: cfg(linux) → cfg(unix) for isatty/ioctl/statvfs
- BUILD_TARGET detects freebsd/openbsd/macos

### Added — Static musl Binary
- release.sh builds gnu + musl binaries with separate tar.gz + checksums
- musl = zero dynamic deps (Alpine, embedded, any Linux)

### Added — GitHub Actions CI
- musl-build job: builds + verifies + smoke tests static binary

### Architecture Policy
- Release binaries: amd64 Linux only (gnu + musl)
- Source: compiles on FreeBSD/OpenBSD/macOS
- Windows: NOT supported

### Verified
- 21/21 depth audit tests PASS
- build.sh check-all PASS, codespell PASS, yamllint PASS, golden 3/3 PASS

## [v13.3.0] — 2026-07-01

### Ecosystem Integration
- Audit log (JSONL) — ~/.local/share/zacxiom/audit.log
- Cron-friendly --quiet flag
- Desktop notifications via notify-send
- Pacman post-transaction hook (opt-in)
- Backup integration docs (restic/borg/rsync)

## [v13.2.0] — 2026-07-01

### Intelligence & Safety
- Colorblind mode (--colorblind) — shapes instead of colors
- Snapshot integrity check (zacxiom snapshot verify)
- Learning risk model — undo flags restored paths
- Smart suggestions (scan --suggest)
- Dry-run diff (clean --dry-run --diff)

## [v13.1.0] — 2026-07-01

### Performance & Polish
- Shell completions (bash/zsh/fish/elvish/powershell)
- Progress ETA
- Incremental scan cache (--no-cache to disable)
- Parallel snapshot pruning (auto_prune_async)

## [v13.0.0] — 2026-06-30

### Masterclass Safety Overhaul — "Safe filesystem cleaning, explained."

### Tagline
- New: "Safe filesystem cleaning, explained."
- Subtitle: "Clean safely. Explain every decision. Recover anything."

### Added — User-Controlled Safety
- `--exclude` flag on scan/clean/simulate/report (glob + directory paths)
- `~/.config/zacxiom/config.toml` — TOML config with strict validation
- `--testconf` global flag — validate config, exit 0 (ok) or 1 (invalid)
- `config` subcommand: `init`, `show`, `path`, `testconf`
- `[rules_exclude].exclude` — config-driven file protection (replaces hardcoded extensions)
- Human-readable sizes: `max_auto_clean_size = "100MB"` (MB/GB only, raw bytes backward-compat)
- `--yes` flag — skip prompts for CI/scripts
- `--include` whitelist mode — only clean matching patterns
- `--fail-fast` flag — stop on first error
- `.zacxiomignore` file support (like `.gitignore`, directory-scoped patterns)
- `example/config.toml` — fully documented, auto-installed by `install.sh`

### Added — Safety Hardening
- Default dry-run on first use (Option A — zero data loss risk for new users)
- Confirmation prompts: `--smart` requires "yes", `--force` requires "DELETE"
- `--force` NO LONGER allows HighRisk files (was cleanable, caused data loss)
- Engine-protected category enforcement: `.git/HEAD`, system binaries, SSH keys → always Protected
- 20 protected extensions by default: `.iso .vmdk .vdi .qcow2 .ova .img .raw .wim .pem .key .p12 .pfx .keystore .jks .gpg .asc`
- Canonical path matching for protected paths (symlink traversal blocked)
- TOCTOU hardening: `O_NOFOLLOW` + `fstat` before rename (prevents symlink swap attacks)
- Atomic cross-fs copy: copy → fsync → verify size/checksum → remove original
- Size-based protection: large files in user dirs need explicit `--force`

### Added — Performance
- Smart threading: 75% of CPUs, scaled by workload, load-aware (reads `/proc/loadavg`)
- `[scan].max_threads` config option (0 = auto, 1-N = manual)
- Progress bar during clean for >100 files

### Added — Recovery
- XDG-compliant snapshot storage: `~/.local/share/zacxiom/snapshots` (was `~/.cache/`)
- SHA-256 trash paths (128-bit collision resistance, was 64-bit DefaultHasher)
- Collision-proof snapshot IDs: `snap-{PID}-{timestamp}-{entropy}`
- Backward compat: legacy `~/.cache/zacxiom/` snapshots still readable
- `[trash].verify_checksum` config option — SHA-256 verify on cross-fs copies

### Fixed — Critical Bugs
- `~/Downloads` removed from default scan roots (root cause of ISO/data loss)
- `safety::validate_clean()` wired into clean pipeline (was dead code)
- `trailing_var_arg` removed from CLI — flags now work after positional paths
- `.git/HEAD` now classified as Protected (was Moderate, cleanable with `--force`)
- Rayon pool `.expect()` replaced with graceful sequential fallback

### Changed
- Tagline: "Easy to use intelligence cleaning in userspace" → "Safe filesystem cleaning, explained."
- HighRisk decision: NEVER cleanable (was cleanable with `--force`)
- Snapshot/trash directory: `~/.cache/zacxiom/` → `~/.local/share/zacxiom/` (XDG compliance)
- `cleaner::clean()` refactored to use `CleanOptions` struct
- `optimal_threads()` now load-aware + config-driven

### Verified
- 402 unit tests PASS
- 30 stress tests PASS (crash recovery, concurrent, permissions, special files)
- 20 adversarial tests PASS (symlink, path traversal, config injection, extension bypass)
- codespell PASS, golden 3/3 PASS, clippy clean
- Zero panics possible in production code

## [v12.0.0] — 2026-06-28

### Added — Interrupt Recovery
- Snapshot saved incrementally during clean — survives kill -9 mid-clean.
  Previously snapshot was written only after ALL files were moved.
  Now each successful file move immediately persists to the snapshot file.
  If process is killed, partial snapshot allows undo of moved files.

### Validated (destructive testing)
- Symlink attack: `cache → /etc` never followed or cleaned
- Parallel stress: 10 concurrent scans, no panics, no deadlocks
- Permission nightmare: chmod 0, root-owned files handled gracefully
- Undo integrity: sha256 before/after clean→undo verified identical
- Fuzz paths: emoji, unicode, 200-char filenames, spaces — no panics
- Cross-device: tmpfs rename → copy+remove fallback verified

### Changed
- Category display: "Developer Tools"→"Developer Cache", "User Data"→"Application Cache"
- cleaner::clean() accepts snapshot reference for incremental saves
- Clean snapshot now persisted before any output (minimizes data loss window)

## [v11.1.1] — 2026-06-28

### Fixed
- Version consistency: single authoritative version source (Cargo.toml)
- Category display names: "Developer Tools" → "Developer Cache",
  "User Data" → "Application Cache" (more accurate for regenerable cache)
- Snapshot "not found" error now shows actionable message

### Changed
- Removed dead `execute_sequentially()` deletion path — single deletion engine
- Full classifier now checks active environments (parity with classify_fast)
- TOCTOU hardening: re-stats files at move time, records actual sizes
- SHA-256 hash-based trash filenames (avoids NAME_MAX for deep paths)
- Snapshot records actual bytes moved, not scanned estimates

## [v11.0.0] — 2026-06-28

### Added — Active Environment Protection
- **Active Environment Protection**: Zacxiom now detects active developer environments
  (Rust toolchains, Python venvs, Node.js runtimes, Go SDKs, Java JDKs, and more)
  before building the clean plan. Active environments are NEVER cleaned.
- New decision tier: `ProtectedActiveEnvironment` — risk ★★★★★ Critical.
  "Never clean what the developer is actively using."
- Environment detectors for: Rust (rustup, cargo), Python (venv, conda, pyenv, uv),
  Node.js (nvm, fnm, volta), Go (GOROOT/GOPATH), Java (JAVA_HOME, sdkman),
  Bun, Deno, Zig, LLVM, and cargo-installed binaries.
- Recently-used file protection (24h default window).

### Added — Snapshot Management
- `zacxiom snapshot list` — list all snapshots with ID, size, creation date, age.
- `zacxiom snapshot delete <id>` — delete a single snapshot.
- `zacxiom snapshot prune --keep N` — keep newest N, delete older.
- `zacxiom snapshot prune --older-than 30d` — age-based pruning.
- `zacxiom snapshot purge --confirm "DELETE ALL"` — delete ALL snapshots
  and trash files. Requires exact confirmation string (no yes/no).
- Snapshot age and size calculation methods.

### Added — Storage Reporting
- `zacxiom status` now displays snapshot count and total disk usage.

### Changed
- `Decision` enum gains `ProtectedActiveEnvironment` variant — never cleanable.
- `Category` gains `ProtectedActiveEnvironment` variant — always protected.
- Classifier pipeline (`classify_fast` + `classify`) checks active environments
  before any other classification.
- Pipeline decision override ensures active environments are never downgraded
  by later classification layers.

## [v10.0.0] — 2026-06-27

### Added
- Trash-based undo system — every `clean` creates a recoverable snapshot
- `snapshot::list_all()` sorted newest-first by modification time
- Categorized error summary in clean output (Permission denied, Read-only, etc.)
- `--system` flag on install and uninstall scripts
- Gatekeeper script (`scripts/gatekeeper.sh`) for pre-commit quality gates
- Release packaging script (`scripts/release.sh`) with SHA-512 verification
- Codespell configuration (`.codespellrc`)
- `RELEASE_CHECKLIST.md`

### Changed
- CLI flag collision resolved: `-P` for `--paths`, `-p` for `--profile`
- Relative paths resolved to absolute in `explain` and `plan`
- Lazy procfs scan — `OnceLock` initialization for 10x startup improvement
- `dpkg -S` query limited to system paths for 10x classification speedup
- Snapshot creation skipped when no files are removed
- `plan` blocked path exits 0 (valid safety refusal = success)
- `undo` failed/load errors exit 1 (actual failure)
- `clean --dry-run --json` produces valid JSON

### Fixed
- Undo bug: always restored 0 files (trash paths now recorded in snapshots)
- `--force` flag respected for `Decision::HighRisk` files
- Scanner no longer follows symlinks during discovery
- Cross-filesystem restore uses copy+remove fallback
- Status "Last snap" shows correct newest snapshot
- Help text for `undo` and `status` subcommands

---

## [v6.x] — 2026-06-22

Early releases. See `docs/` for historical audit reports.

---

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
