<p align="center">
  <img src="assets/zacxiom-logo-master.png" alt="zacxiom logo" width="260">
</p>

<h1 align="center">zacxiom</h1>

<p align="center">
  <strong>Filesystem intelligence for Linux — Observe → Understand → Decide → Act</strong>
</p>

<p align="center">
  A safe-by-default filesystem intelligence engine that evaluates risk, explains decisions,
  and reclaims space only when it is safe to do so. Every deletion is recoverable.
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

## Quick Start

```bash
# Install
./scripts/install.sh            # user install → ~/.local/bin
./scripts/install.sh --system   # system install → /usr/local/bin (needs sudo)

# Uninstall
./scripts/uninstall.sh          # user uninstall
./scripts/uninstall.sh --system # system uninstall (needs sudo)

# Scan — what exists, what is safe
zacxiom scan

# Explain — why is a specific path safe or blocked?
zacxiom explain ~/.cache

# Plan — what is safe and recommended? (read-only)
zacxiom plan

# Simulate — what would happen?
zacxiom simulate

# Clean — safe files only
zacxiom clean

# Clean — safe + low risk
zacxiom clean --smart

# Clean — with confirmation
zacxiom clean --force

# Undo — restore files from last cleanup
zacxiom undo

# Undo — restore from specific snapshot
zacxiom undo --id snap-xxxx

# Status — system health and snapshot overview
zacxiom status

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

## Intelligence Layers

| Layer | Capability |
|-------|-----------|
| **Domain Summary** | Cache categorized by type — browser, build, system, package |
| **Decision Summary** | Files found, safe to clean, blocked, recoverable |
| **Risk Engine** | 7-signal scoring — age, process, ownership, regenerability, path, history, memory |
| **Simulation** | Mandatory dry-run with action labels: WOULD CLEAN, BLOCKED, NEVER |
| **Context Memory** | Adaptive thresholds per system — learns what you trust |
| **Safety Lock** | H1–H6 hard rules enforced at runtime — no bypass |

## Safety Guarantees

- **H1** — No silent deletion. Every action requires explicit intent.
- **H2** — System paths hard-protected (never removable).
- **H3** — Every action is logged for audit.
- **H4** — No root required for operation.
- **H5** — Simulation mandatory before clean.
- **H6** — `--force` requires explicit `YES` confirmation.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for module structure, data flow, and engine design.

See [docs/RULES.md](docs/RULES.md) for the complete hardened safety specification.

## Release Verification

```bash
# Verify release integrity
sha512sum -c zacxiom-v12.0.1-linux-amd64.tar.gz.sha512
```

## Intellectual Property

**zacxiom** is created and maintained by **rezky_nightky (oxyzenQ)**.

The name, logo, and brand identity are protected trademarks.
Forks and derivatives must use distinct branding per the GPL-3.0 license terms.
See [TRADEMARK.md](TRADEMARK.md) for full terms.

## License

Source code: **GPL-3.0-only** — see [LICENSE](LICENSE)

---

<p align="center">
  <sub>© 2026 rezky_nightky (oxyzenQ). Built with Rust. Designed for Linux.</sub>
</p>
