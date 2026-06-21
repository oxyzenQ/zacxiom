# 🛡️ ZACXIOM RULES — Hardened Safety Specification

> These rules are **non-negotiable**.
> They define what Zacxiom CAN do, MUST do, and MUST NEVER do.
> Core engine must enforce them — no plugin can override.

---

## ⚫ RULE 0: THE PRIME DIRECTIVE

```text
Zacxiom's primary goal is correctness of decision, not amount of space freed.
A correct "do nothing" is better than an incorrect deletion.
```

---

## 🔴 NON-OVERRIDABLE HARD RULES (ALL VERSIONS)

### H1 — No Silent Deletion
> Every file removal MUST be preceded by explicit user intent.
> `simulate` MUST always run before `clean` — even with `--force`.

### H2 — Protected Paths (Hard-coded, Never Removable)
```
/boot/**
/etc/**
/sys/**
/proc/**
/dev/**
/bin/**
/sbin/**
/lib/**
/lib64/**
/usr/bin/**
/usr/sbin/**
/usr/lib/**
/usr/lib64/**
/usr/include/**
/usr/share/**
/var/lib/dpkg/**
/var/lib/rpm/**
/var/lib/pacman/**
/home/*/.ssh/**
/home/*/.gnupg/**
```
> Any file under these paths → `Decision::Protected` → blocked at risk engine level.

### H3 — No Unlogged Action
> Every scan, simulation, and clean operation MUST produce structured output.
> Output format: `file → reason → risk → decision`

### H4 — No Root Assumption
> Zacxiom works as unprivileged user by default.
> Root is allowed but never required.

### H5 — No External Mutation Without Simulation
> `clean` without prior `simulate` → rejected.
> Simulation report MUST be shown before any deletion.

### H6 — Force Mode Gating
> `--force` flag requires:
> 1. `simulate` output displayed
> 2. Explicit interactive confirmation: `"Type YES to proceed"`
> 3. Confirmation is case-sensitive, exact match required

---

## 🟡 RISK CLASSIFICATION RULES (v5.1.0)

### R1 — Safe (score 0.0–0.2)
- File is in a known cache directory
- File is owned by user
- File is not open by any process
- File type is regular file (not symlink to system path)

→ `Decision::Safe`

### R2 — Low Risk (score 0.2–0.4)
- Cache file in system directory (`/var/cache/*`)
- Old log files not currently written to
- Orphan files not matching any package

→ `Decision::LowRisk` (requires `--smart`)

### R3 — Moderate Risk (score 0.4–0.7)
- File owned by a package but marked as "config" or "data"
- Recently modified (< 7 days)
- File in a shared directory (`/opt`, `/usr/local`)

→ `Decision::Moderate` (requires `--force`)

### R4 — High Risk (score 0.7–0.9)
- File currently open by a running process
- File in a system directory without clear cache classification
- Symlink targeting system path

→ `Decision::HighRisk` (blocked by default, `--force` with 2nd confirmation)

### R5 — Protected (score 0.9–1.0)
- File matches H2 protected paths
- Critical system binary
- Authentication/credential file

→ `Decision::Protected` (never deletable, even with `--force`)

---

## 📋 DECISION MATRIX

| `--flag` | Safe | LowRisk | Moderate | HighRisk | Protected |
|----------|------|---------|----------|----------|-----------|
| (none) | ✅ clean | ❌ skip | ❌ skip | ❌ skip | ❌ blocked |
| `--smart` | ✅ clean | ✅ clean | ❌ skip | ❌ skip | ❌ blocked |
| `--force` | ✅ clean | ✅ clean | ✅ clean* | ⚠️ 2nd confirm | ❌ blocked |

> `*` = requires `YES` confirmation
> `⚠️` = requires `YES --force` confirmation

---

## 📤 OUTPUT STANDARD (ALL VERSIONS)

Every entry in simulation/clean report MUST contain:

```text
[PATH] → [CACHE_DOMAIN] → [OWNERSHIP] → [RISK_SCORE] → [DECISION] → [REASON]
```

Example:
```text
~/.cache/mozilla/firefox/abc123/cache2/entries/ → browser → user → 0.00 → SAFE → "Browser cache, user-owned, not in use"
/var/cache/apt/archives/lock → package_manager → system → 0.05 → SAFE → "Package manager cache lock, not critical"
/etc/nginx/nginx.conf → system → package(nginx) → 0.95 → PROTECTED → "System config, H2 protected path"
```

---

## 🧪 COMPLIANCE CHECKS (`build.sh check-all`)

```bash
cargo fmt --all -- --check   # Style
cargo clippy -- -D warnings  # Lint (no warnings allowed)
cargo test                   # All tests must pass
cargo audit                  # Dependency vulnerabilities (when available)
```

> CI fails if any rule is violated. No exceptions.

---

## ⚙️ ENGINEERING RULES (Masterclass)

### E1 — Core LOC Constraint
> Core engine MUST remain under 1,000 lines of Rust code (`src/*.rs`).
> Excludes `*.md`, `*.txt`, test fixtures, and build scripts.
> If a single file exceeds 400 LOC, it MUST be decomposed.

### E2 — `main.rs` Purity
> `main.rs` MUST only contain bootstrap, wiring, and dispatch (target: <200 LOC).
> Logic goes into domain modules: `scanner`, `cache`, `ownership`, `risk`, etc.

### E3 — Version Output
> `-V` / `--version` MUST follow the masterclass format:
> ```
> zacxiom -V/--version
> Version: vX.Y.Z
> Build: linux-x86_64 (git-hash)
> Copyright: (c) 2026 rezky_nightky (oxyzenQ)
> License: GPL-3.0
> Source: https://github.com/oxyzenQ/zacxiom
> ```

### E4 — Gatekeeper Script
> `./build.sh check-all` MUST pass before every commit.
> Sequence: fmt → clippy → build → test → audit.
> Exit immediately on any hard failure.

### E5 — Version Bumping
> `./version-to vX.Y.Z` is the single source of truth for version bumps.
> No manual version edits allowed.

### E6 — Release Profile
> `[profile.release]` MUST optimize for stability and efficiency:
> ```toml
> opt-level = 3
> debug = false
> strip = true
> lto = "thin"
> codegen-units = 1
> ```

## 🔮 RULES EVOLUTION (v2 → v5)

| Version | New Rules |
|---------|-----------|
| v2 | Process-aware protection (R4 extended), history tracking constraints |
| v3 | Context graph validation, profile-specific rules |
| v4 | User policy engine (must not override H-rules), snapshot metadata integrity |
| v5 | Rules frozen — no new H-rules, only refinement of R-rules |
