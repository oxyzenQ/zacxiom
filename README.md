<p align="center">
  <img src="assets/zacxiom-logo-master.png" alt="zacxiom logo" width="260">
</p>

<h1 align="center">zacxiom</h1>

<p align="center">
  <strong>Filesystem intelligence for Linux. Observe → Understand → Decide → Act<strong>
</p>

<p align="center">
Safe-by-default filesystem intelligence engine that evaluates risk, explains decisions, and reclaims space only when it is safe to do so.

</p>

<p align="center">
  <a href="https://ko-fi.com/rezky">
    <img src="https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827" alt="Support on Ko-fi">
  </a>
</p>

---

## Philosophy

zacxiom follows four principles.

### 1. Safety Before Space

Recovering disk space is secondary. The primary objective is preventing incorrect deletion.

### 2. Explainability By Default

Every recommendation includes reason, risk, and decision. No silent actions. No hidden logic.

### 3. Context Matters

A file is never evaluated in isolation. Risk assessment considers filesystem location, ownership, process activity, regenerability, and system impact.

### 4. Observe Before Acting

The zacxiom model is **Observe → Understand → Decide → Act**. Never skip directly from observing to deleting.

---

## Quick Start

```bash
# Build
./scripts/build.sh check-all

# Scan — analytical view: what exists, what is safe
zacxiom scan

# Simulate — operational view: what would happen
zacxiom simulate

# Safe clean only
zacxiom clean

# Smart clean (safe + low risk)
zacxiom clean --smart

# Force clean (requires explicit YES confirmation)
zacxiom clean --force

# Undo last clean operation
zacxiom undo

# System status overview
zacxiom status
```

## Intelligence Layers

| Layer | Capability |
|-------|-----------|
| **Domain Summary** | Cache categorized by type — browser, build, system, package |
| **Decision Summary** | Immediate answer: files found, safe to clean, blocked, recoverable |
| **Risk Engine** | 7-signal scoring — age, process, ownership, regenerability, path, history, memory |
| **Simulation** | Mandatory dry-run with action labels: WOULD CLEAN, BLOCKED, NEVER |
| **Context Memory** | Adaptive thresholds per system — learns what you trust |
| **Safety Lock** | H1–H6 hard rules enforced at runtime — no bypass |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for module structure and data flow.

See [docs/RULES.md](docs/RULES.md) for the complete hardened safety specification.

## Safety Guarantees

- **H1** — No silent deletion
- **H2** — System paths hard-protected (never removable)
- **H3** — Every action is logged
- **H4** — No root required
- **H5** — Simulation mandatory before clean
- **H6** — `--force` requires explicit `YES` confirmation

---

## Version

```text
zacxiom v7.0.0
Build: linux-x86_64
Copyright: (c) 2026 rezky_nightky (oxyzenQ)
License: GPL-3.0
Source: https://github.com/oxyzenQ/zacxiom
```

## Intellectual Property

zacxiom is the exclusive intellectual property of **rezky_nightky (oxyzenQ)**.
This project is not for sale. Source code is provided under GPL-3.0.
See [docs/trademark.md](docs/trademark.md) for full terms.

## License

GPL-3.0-only — see [LICENSE](LICENSE)
