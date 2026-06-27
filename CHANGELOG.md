# Changelog

All notable changes to zacxiom.

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
