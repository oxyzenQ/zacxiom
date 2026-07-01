# 🗺️ Zacxiom Future Roadmap

> **Status:** v14.0.0 released. This document is for future development when you return.
> **Last updated:** 2026-07-01
> **Maintainer:** rezky_nightky (oxyzenQ)

---

## 🔒 Locked Principles (never change)

| Principle | Detail |
|-----------|--------|
| **Linux first** | amd64 Linux binaries. BSD/macOS source support. Windows = never |
| **No daemon/TUI/API** | CLI + config + exit codes only |
| **No bloat** | If it doesn't serve safety/explainability/recovery, reject it |
| **Config-driven** | Extend `[rules_exclude]` pattern, not hardcode |
| **Community handles distribution** | AUR/Nix/Homebrew when project is popular enough |
| **Approve-gated releases** | Every release waits for maintainer approval |

---

## ✅ Completed Phases

| Version | Phase | Highlights |
|---------|-------|-----------|
| v13.0.0 | Masterclass Safety | --exclude, config.toml, --testconf, rules_exclude, human-readable sizes |
| v13.1.0 | Performance & Polish | Shell completions, progress ETA, incremental cache, parallel pruning |
| v13.2.0 | Intelligence & Safety | Colorblind mode, snapshot verify, learning risk model, smart suggestions |
| v13.3.0 | Ecosystem Integration | Audit log, --quiet, desktop notifications, pacman hook, backup docs |
| v14.0.0 | Cross-Unix + Cache Engine | musl static binary, cache-aware classification (64x CPU reduction), BSD/macOS source |

---

## 📅 Future Roadmap (when you return)

### Phase 5: v14.1.0 — **Polish & Stability** (low effort, high value)

Pick these up anytime. Each is isolated and low-risk.

| Feature | Why | Complexity | File to touch |
|---------|-----|-----------|---------------|
| **Man page generation** | `clap_mangen` dep → `zacxiom man` outputs groff | Low | cli.rs, main.rs |
| **Streaming scan** | Yield files as found (memory efficient for 1M+ files) | Medium | scanner.rs |
| **Cache compression** | scan_cache.json gets big (85k files = 15MB). Add zstd compression | Low | scan_cache.rs |
| **`--cache-stats` flag** | Show cache size, hit rate, last-updated without scanning | Low | commands/scan.rs |
| **Config hot-reload** | Watch config.toml for changes, auto-reload (inotify) | Medium | config.rs |
| **Progress bar for clean** | Show "Cleaning 50/200 (25%)" during deletion | Low | cleaner.rs |

**Goal:** Make zacxiom feel polished and production-ready for daily driver use.

---

### Phase 6: v14.2.0 — **Advanced Intelligence** (medium effort)

| Feature | Why | Complexity |
|---------|-----|-----------|
| **File usage tracking (opt-in)** | Track last-accessed times for better age scoring. Use `statx()` on Linux (atime unreliable with relatime) | Medium |
| **Predictive cleanup** | "Based on your patterns, ~500MB reclaimable weekly. Run `zacxiom clean --smart --yes`?" | Medium |
| **Duplicate detection** | Find duplicate files (by hash) across cache dirs — "dedup before clean" | High |
| **Space visualization** | `zacxiom viz` — ASCII treemap of disk usage (like dust/ncdu) | Medium |
| **Age-based policies** | `[clean].auto_clean_older_than = "90d"` — auto-clean files older than 90 days | Medium |
| **Per-domain config** | `[domain.browser] max_age = "7d"`, `[domain.build] max_age = "30d"` | Medium |

**Goal:** zacxiom becomes proactive — predicts, visualizes, auto-maintains.

---

### Phase 7: v14.3.0 — **Enterprise Readiness** (opt-in, not default)

| Feature | Why | Complexity |
|---------|-----|-----------|
| **Structured logging (tracing)** | Replace eprintln! with `tracing` crate for structured logs | Medium |
| **Prometheus metrics export** | `zacxiom --metrics` → text file for node_exporter | Low |
| **Config profiles** | `zacxiom --profile dev` vs `zacxiom --profile server` — different defaults | Medium |
| **Snapshot encryption** | Encrypt trash copies for sensitive environments (age/gpg) | High |
| **Audit log rotation** | Auto-rotate audit.log when > 100MB | Low |
| **SELinux/AppArmor profiles** | Ship security policy files for sandboxed deployment | Medium |

**Goal:** zacxiom fits enterprise/compliance environments without changing defaults.

---

### Phase 8: v15.0.0 — **Ecosystem** (when community grows)

| Feature | Why | Complexity |
|---------|-----|-----------|
| **Plugin system (Lua)** | Users write custom rules in Lua (sandboxed, no WASM complexity) | Very High |
| **Community rule marketplace** | Share `[rules_exclude]` presets — "gaming", "dev", "minimal" | Medium |
| **REST API (opt-in daemon)** | `zacxiom serve` for remote management (homelab) | High |
| **Web dashboard** | Simple HTML UI for browsing scan results + triggering clean | Medium |
| **Multi-user policies** | System-wide config in `/etc/zacxiom/` — admin controls | Medium |

**Goal:** zacxiom becomes extensible platform, not just a tool.

---

## 🚫 Explicitly Rejected (locked)

| Feature | Why |
|---------|-----|
| ~~Windows support~~ | Different filesystem philosophy, not worth the effort |
| ~~GPU acceleration~~ | I/O bound, not CPU. Adds 200MB deps for zero benefit |
| ~~Background daemon (always-on)~~ | Violates "no daemon" principle. Opt-in only |
| ~~AI/ML classification~~ | Rule engine is 99.9% accurate. ML adds opacity (violates "explained") |
| ~~Enterprise RBAC~~ | Too complex for a filesystem cleaner. Use OS permissions |
| ~~WASM plugins~~ | Lua is simpler. WASM adds 10MB runtime for no benefit |
| ~~Cloud sync~~ | Out of scope — local tool, not cloud service |
| ~~Native GUI (Qt/GTK)~~ | TUI + CLI covers this. Native GUI = maintenance burden |

---

## 🔄 Maintenance Tasks (do periodically)

| Task | Frequency | How |
|------|-----------|-----|
| **Update dependencies** | Monthly | `cargo update`, run `./scripts/build.sh check-all` |
| **Audit for vulnerabilities** | Monthly | `cargo audit` (install: `cargo install cargo-audit`) |
| **Prune old CI caches** | Quarterly | GitHub Actions → clear cache |
| **Review issues/PRs** | Weekly | github.com/oxyzenQ/zacxiom/issues |
| **Update CHANGELOG** | Every release | Add entry before tagging |

---

## 📊 Current State (v14.0.0)

| Metric | Value |
|--------|-------|
| Version | v14.0.0 |
| Tests | 413 passing |
| Lines of code | ~15k (src/) |
| Dependencies | 10 (clap, walkdir, serde, serde_json, libc, rayon, toml, globset, sha2, clap_complete) |
| Binary size | 1.6 MB (gnu), 1.7 MB (musl static) |
| Cache hit rate | 100% on unchanged files |
| CPU reduction | 64x on warm scans |

---

## 🚀 How to Resume

When you return:

1. `cd zacxiom && git pull`
2. `. "$HOME/.cargo/env"`
3. `./scripts/build.sh check-all` — verify everything still works
4. Pick a phase from this roadmap
5. Create branch: `git checkout -b feat-v14.1`
6. Implement, test, commit
7. Tag release only after approval

**Quick start commands:**
```bash
cd ~/zacxiom
. "$HOME/.cargo/env"
cargo build --release
./target/release/zacxiom --version
./scripts/build.sh check-all
```

---

## 📝 Notes for Future Self

- **Cache version is 2** — if you change `CachedFile` struct, bump `CACHE_VERSION` to invalidate old caches
- **Golden tests** must be regenerated when CLI output changes: `./scripts/golden-update.sh`
- **codespell + yamllint** run in CI — fix locally before push
- **musl target** already installed: `rustup target list --installed | grep musl`
- **Release workflow** auto-builds gnu + musl on tag push — just tag and push
- **`--no-cache` flag** exists for forced reclassification
- **Snapshot IDs** are collision-proof: `snap-{PID}-{timestamp}-{entropy}`

---

*Built with Rust. Designed for Linux. Maintained by rezky_nightky (oxyzenQ).*
*Safe filesystem cleaning, explained.*
