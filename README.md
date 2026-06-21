# zacxiom — Filesystem Intelligence Engine

**Observe → Understand → Decide → Act**

[![Ko-fi](https://img.shields.io/badge/Ko--fi-rezky-ff5e5b?logo=kofi&logoColor=white)](https://ko-fi.com/rezky)

Safe-by-default filesystem intelligence engine for Linux. Not a cleaner — an intelligence system that happens to reclaim space safely. Every decision is justified. Every action is logged.

## Philosophy

```text
Zacxiom's primary goal is correctness of decision, not amount of space freed.
A correct "do nothing" is better than an incorrect deletion.
```

## Quick Start

```bash
# Build
./build.sh check-all

# Scan for cache files
zacxiom scan

# Simulate — always run before clean
zacxiom simulate

# Safe clean only
zacxiom clean

# Smart clean (safe + low risk)
zacxiom clean --smart

# Force clean (requires explicit YES confirmation)
zacxiom clean --force
```

## Output Standard

Every file in every report follows:
```
file → reason → risk → decision
```

## Safety Guarantees

- **H1** — No silent deletion
- **H2** — System paths hard-protected (never removable)
- **H3** — Every action is logged
- **H4** — No root required
- **H5** — Simulation mandatory before clean
- **H6** — `--force` requires explicit `YES` confirmation

See [RULES.md](RULES.md) for the complete hardened safety specification.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for module structure, data flow, and LOC budget.

## Version

```text
zacxiom v5.2.0
Build: linux-x86_64
Copyright: (c) 2026 rezky_nightky (oxyzenQ)
License: GPL-3.0
Source: https://github.com/oxyzenQ/zacxiom
```

## License

GPL-3.0-only — see [LICENSE](LICENSE)
