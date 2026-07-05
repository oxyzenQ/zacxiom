<p align="center">
  <img src="assets/zacxiom-logo-master.png" alt="zacxiom logo" width="260">
</p>

<h1 align="center">zacxiom</h1>

<p align="center">
  <strong>Safe filesystem cleaning, explained.</strong>
</p>

<p align="center">
  Clean safely. Explain every decision. Recover anything.
</p>

<p align="center">
  <a href="https://ko-fi.com/rezky"><img src="https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827" alt="Support"></a>
</p>

---

## Philosophy

zacxiom follows four principles:

1. **Safety Before Space** — Recovering disk space is secondary. The primary objective is preventing incorrect deletion.
2. **Explainability By Default** — Every recommendation includes reason, risk, and decision. No silent actions.
3. **Context Matters** — Files are evaluated in context: location, ownership, process activity, regenerability.
4. **Observe Before Acting** — `Observe → Understand → Decide → Act`. Never skip directly to deleting.

## How zacxiom Compares

zacxiom is the only filesystem cleaner that combines cache-aware classification, trash-based recovery, explainability, and compliance-grade audit logging — all in a static musl binary with zero dependencies.

### Feature Comparison

| Feature | zacxiom v14 | BleachBit 6.0 | ncdu 2.9 | dust 1.2 | rmlint 2.10 | fdupes 2.4 |
|---------|:-----------:|:-------------:|:--------:|:--------:|:-----------:|:----------:|
| **Trash-based undo** | ✅ Built-in | ❌ Permanent | ❌ Permanent | N/A | ⚠️ Via script | ❌ Permanent |
| **Cache-aware scan** | ✅ 100% hit | ❌ Full rescan | ⚠️ Export only | ❌ | ✅ Replay | ✅ DB |
| **Config-driven rules** | ✅ TOML | ⚠️ XML (dev) | ❌ | ❌ | ⚠️ CLI flags | ❌ |
| **Human-readable sizes** | ✅ `"100MB"` | ❌ Raw bytes | ❌ | ❌ | ❌ | ❌ |
| **Per-file explainability** | ✅ Reasons | ⚠️ File list | ❌ | ❌ | ✅ sh+json | ⚠️ Dupe groups |
| **Audit log (JSONL)** | ✅ Compliance | ❌ | ❌ | ❌ | ⚠️ JSON | ⚠️ Plain text |
| **Load-aware threading** | ✅ Adaptive | ❌ Single | ❌ Fixed | ❌ Fixed | ❌ Fixed | ❌ Single |
| **Colorblind mode** | ✅ Shapes | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| **Learning (undo patterns)** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Config validation** | ✅ `--testconf` | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Snapshot verify** | ✅ Integrity | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Static musl binary** | ✅ Zero deps | ❌ Python+GTK | ✅ | ⚠️ Static-ish | ⚠️ Buildable | ⚠️ Buildable |
| **Binary size** | 1.6 MB | ~50-100 MB | ~250 KB | ~2 MB | ~500 KB | ~150 KB |

### What Makes zacxiom Unique

No single competitor offers this combination:

- **Cache-aware classification** — 64x CPU reduction on repeat scans (100% cache hit confirmed in production with 70k files)
- **Native trash-based undo** — `zacxiom undo` restores any deletion; competitors delete permanently
- **Explainability + audit log together** — per-file reasons + compliance-grade JSONL logging
- **Human-readable TOML config** — `max_auto_clean_size = "100MB"` (not `104857600`)
- **Load-aware adaptive threading** — reads `/proc/loadavg`, reduces threads when system is busy
- **Accessibility** — colorblind mode with shape-based indicators
- **Zero dependencies** — static musl binary works on Alpine, embedded, any Linux

### Where Competitors Still Win

- **BleachBit** — broader cleaning coverage (1000+ targets via CleanerML), cookie manager, free-space wipe
- **ncdu** — ubiquity (every sysadmin knows it), 250 KB footprint
- **dust** — zero-config analyzer UX with colored tree visualization
- **rmlint** — raw dedup power (broken symlinks, nonstripped binaries, duplicate directories)

### Performance (Real-World, 70k files)

| Metric | zacxiom v14 | Best Competitor |
|--------|:-----------:|:---------------:|
| Cold scan | 7.0s | BleachBit: ~30s+ (Python, single-thread) |
| Warm scan (cached) | **2.4s** | N/A — no competitor has cache |
| CPU on warm scan | **0.006s** | N/A |
| Recovery after delete | `zacxiom undo` | BleachBit: ❌ permanent |

---

## Quick Start

```bash
# Install
./scripts/install.sh            # user install → ~/.local/bin (also copies example config)
./scripts/install.sh --system   # system install → /usr/bin (needs sudo)

# Uninstall
./scripts/uninstall.sh          # user uninstall
./scripts/uninstall.sh --system # system uninstall (needs sudo)

# Scan — what exists, what is safe
zacxiom scan

# Scan with exclude — protect specific paths/patterns
zacxiom scan --exclude "~/Downloads" --exclude "*.iso"

# Explain — why is a specific path safe or blocked?
zacxiom explain ~/.cache

# Plan — what is safe and recommended? (read-only)
zacxiom plan

# Simulate — what would happen?
zacxiom simulate

# Clean — safe files only (first run = dry-run preview)
zacxiom clean

# Clean — actually delete (skip dry-run + prompts)
zacxiom clean --yes

# Clean — safe + low risk
zacxiom clean --smart --yes

# Clean — with confirmation (type "DELETE")
zacxiom clean --force

# Clean — whitelist mode (only clean matching patterns)
zacxiom clean --include "target/*" --include "node_modules/*" --smart --yes

# Clean — stop on first error
zacxiom clean --smart --yes --fail-fast

# Undo — restore files from last cleanup
zacxiom undo

# Undo — restore from specific snapshot
zacxiom undo --id snap-xxxx

# Status — system health and snapshot overview
zacxiom status

# Configuration
zacxiom config init      # create ~/.config/zacxiom/config.toml
zacxiom config show      # print effective config
zacxiom config path      # print config file location
zacxiom --testconf       # validate config (exit 0 = ok, 1 = invalid)

# Check for updates
zacxiom --check-update
```

## Build from Source

```bash
# Prerequisites: Rust 1.96+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/oxyzenQ/zacxiom.git
cd zacxiom
./scripts/build.sh check-all
```

## v14.3.2 — Enterprise Readiness

### Config Presets
- `--preset dev|server|minimal` — switch between safety profiles instantly
- `server`: conservative (500MB threshold, 7-day snapshot retention)
- `minimal`: ultra-safe (10MB threshold, no age-based auto-clean)

### Prometheus Metrics
- `--metrics` flag exports Prometheus text format for monitoring
- 7 metrics: operations, files removed, bytes freed, errors, etc.
- Compatible with node_exporter textfile collector

### Audit Log Rotation
- Auto-rotates when `audit.log` exceeds 100MB
- Keeps last 1000 entries, archives old as `audit.log.1`

## v14.2.0 — Advanced Intelligence

### Age-Based Auto-Clean
- `[clean].auto_clean_older_than_days = 90` — files older than threshold auto-cleanable

### Duplicate Detection
- `zacxiom dedup` — find duplicate files by size → SHA-256 hash
- Read-only, shows wasted space, JSON output for scripting

### Space Visualization
- `zacxiom viz` — ASCII treemap of disk usage (like dust/ncdu, read-only)

## v14.1.0 — Polish & Stability

### Man Page
- `zacxiom man` — generate groff man page via clap_mangen

### Cache Compression
- Scan cache now gzip-compressed (15MB → ~2MB, 87% reduction)

### Cache Statistics
- `--cache-stats` flag — show cache size, entries, hit rate without scanning

## v14.0.0 — Cross-Unix + Performance

### Performance Fix
- **Scan cache double-scan eliminated** — was doing a second full filesystem scan
  to update cache. Now reuses classification results. Halves I/O on cached scans.

### Cross-Unix Source Support
- **FreeBSD/OpenBSD/macOS** — source compiles, graceful degradation for `/proc`
- `cfg(unix)` replaces `cfg(linux)` for isatty, ioctl, statvfs
- Process awareness disabled on non-Linux (no `/proc`), all other features work

### Static musl Binary
- `release.sh` builds two binaries: glibc (gnu) + static (musl)
- musl = zero dynamic dependencies — works on Alpine, embedded, any Linux
- GitHub Actions CI verifies both builds

### Architecture Policy
- **Release binaries**: amd64 Linux only (gnu + musl)
- **Source**: compiles on FreeBSD, OpenBSD, macOS
- **Windows**: NOT supported (different filesystem philosophy)

## v13.0.0 — What's New

### User-Controlled Safety
- **`--exclude`** — protect specific paths/patterns from scan/clean
- **`~/.config/zacxiom/config.toml`** — TOML config with strict validation
- **`--testconf`** — validate config; malformed = hard error (exit 2)
- **`config init/show/path`** — manage config via CLI
- **`[rules_exclude].exclude`** — config-driven protection (no hardcoded extensions)
- **Human-readable sizes** — `max_auto_clean_size = "100MB"` (MB/GB only)

### Safety Hardening
- **Default dry-run on first use** — zero data loss risk for new users
- **Confirmation prompts** — `--smart` requires "yes", `--force` requires "DELETE"
- **`--yes` flag** — skip prompts for CI/scripts
- **`--force` NO LONGER allows HighRisk** — config/credentials never auto-deleted
- **Engine-protected categories** — `.git/HEAD`, system binaries, SSH keys → always Protected
- **20 protected extensions** — `.iso .vmdk .vdi .qcow2 .ova .img .pem .key` etc.
- **TOCTOU hardening** — `O_NOFOLLOW` + `fstat` prevents symlink swap attacks
- **Atomic cross-fs copy** — fsync + optional SHA-256 checksum verification
- **Canonical path matching** — symlink traversal (`/tmp/link → /etc`) blocked

### Performance
- **Smart threading** — 75% of CPUs, load-aware (reads `/proc/loadavg`), never hogs CPU
- **`[scan].max_threads`** — manual override (0 = auto, 1-N = manual)
- **Progress bar** — shown for >100 file deletions

### Recovery
- **XDG-compliant storage** — snapshots in `~/.local/share/zacxiom/` (not `~/.cache/`)
- **SHA-256 trash paths** — 128-bit collision resistance
- **Collision-proof snapshot IDs** — `snap-{PID}-{timestamp}-{entropy}`
- **Backward compat** — legacy `~/.cache/zacxiom/` snapshots still readable

### Developer Experience
- **`.zacxiomignore`** — project-level exclude patterns (like `.gitignore`)
- **`--include` whitelist mode** — only clean matching patterns
- **`--fail-fast`** — stop on first error
- **`example/config.toml`** — fully documented, auto-installed by `install.sh`

## Configuration

Zacxiom reads `~/.config/zacxiom/config.toml` on startup. If the file has syntax errors or invalid values, zacxiom **refuses to run** — never silently falls back to defaults.

```bash
# Create default config
zacxiom config init

# Validate
zacxiom --testconf

# Show effective config
zacxiom config show
```

Example config sections:
```toml
[scan]
exclude = ["~/Downloads", "~/Documents"]
exclude_patterns = ["*.tmp"]
max_threads = 0  # 0 = auto (75% CPUs, load-aware)

[rules_exclude]
# Files zacxiom NEVER scans or cleans — add your own
exclude = ["*.iso", "*.vmdk", "*.pem", "*.private", "Crypto_wallet.sha256sum"]

[clean]
require_confirmation = true
default_mode = "safe"  # "safe" | "smart" (force NOT allowed as default)
max_auto_clean_size = "100MB"  # human-readable (MB/GB only)
first_run_dry_run = true

[snapshot]
dir = "~/.local/share/zacxiom/snapshots"
auto_prune_days = 30

[trash]
verify_checksum = false  # true = SHA-256 verify on cross-fs copies
```

See [example/config.toml](example/config.toml) for the full documented example.

## Intelligence Layers

| Layer | Capability |
|-------|-----------|
| **Domain Summary** | Cache categorized by type — browser, build, system, package |
| **Decision Summary** | Files found, safe to clean, blocked, recoverable |
| **Risk Engine** | 7-signal scoring — age, process, ownership, regenerability, path, history, memory |
| **Simulation** | Mandatory dry-run with action labels: WOULD CLEAN, BLOCKED, NEVER |
| **Context Memory** | Adaptive thresholds per system — learns what you trust |
| **Safety Lock** | H1–H6 hard rules enforced at runtime — no bypass |
| **Config-Driven Rules** | v13: `[rules_exclude].exclude` — no hardcoded extensions |

## Safety Guarantees

- **H1** — No silent deletion. Every action requires explicit intent.
- **H2** — System paths hard-protected (never removable). Canonical path check blocks symlink traversal.
- **H3** — Every action is logged for audit.
- **H4** — No root required for operation.
- **H5** — Simulation mandatory before clean. First-run auto dry-run.
- **H6** — `--force` requires explicit confirmation (type "DELETE") or `--yes`.
- **v13** — `--force` NO LONGER allows HighRisk files. Protected extensions (`.iso`, `.pem`, etc.) NEVER cleanable. Config validation is strict — malformed = hard error.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for module structure, data flow, and engine design.

See [docs/RULES.md](docs/RULES.md) for the complete hardened safety specification.

## Release Verification

Each release ships **three** checksums: classical SHA-512 + quantum-resistant
BLAKE2b-512 + SHAKE256. Full instructions in
[docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md).

```bash
# Classical (universal)
sha512sum -c zacxiom-vX.Y.Z-linux-amd64-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c zacxiom-vX.Y.Z-linux-amd64-gnu.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via openssl)
openssl dgst -shake256 zacxiom-vX.Y.Z-linux-amd64-gnu.tar.gz
# Compare hash with: cat zacxiom-vX.Y.Z-linux-amd64-gnu.tar.gz.shake256
```

## Intellectual Property & Trademark

**zacxiom** is the exclusive intellectual property of
**rezky_nightky (oxyzenQ)**.

- Source code: licensed under **GPL-3.0-only** (see [LICENSE](LICENSE)).
- Name, logo, and branding ("the Marks"): governed by
  [TRADEMARK.md](TRADEMARK.md). The Marks are NOT covered by the GPL and
  are reserved by the owner.
- This project is **NOT for sale**. Unauthorized rebranding, relicensing,
  or source-code theft is strictly prohibited.

For trademark licensing or written permission, contact
**rezky_nightky (oxyzenQ)** — https://github.com/oxyzenQ.

---

© 2026 rezky_nightky (oxyzenQ). All rights reserved.
