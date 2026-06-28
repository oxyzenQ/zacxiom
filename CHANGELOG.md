# Changelog

All notable changes to zacxiom.

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
