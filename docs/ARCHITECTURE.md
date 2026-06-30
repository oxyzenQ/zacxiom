# 🧱 ZACXIOM ARCHITECTURE v13.0.0

> **Constraint:** Core engine <1k LOC. Plugins unlimited.
> **Law:** If a module doesn't serve safety, explainability, or awareness — it doesn't exist.
> **v13:** User-controlled safety — config-driven rules, no hardcoded extensions.

---

## 📦 Crate Layout

```
zacxiom/
├── Cargo.toml
├── build.sh                  # build + check-all
├── ARCHITECTURE.md
├── RULES.md
├── example/
│   └── config.toml           # v13: fully documented example config
├── src/
│   ├── main.rs               # Entry point — config load, --testconf, dispatch
│   ├── cli.rs                # clap derive (scan/clean/simulate/config + --exclude/--yes/--fail-fast)
│   ├── config.rs             # v13: TOML config + strict validation + human-readable sizes
│   ├── exclude.rs            # v13: ExcludeFilter (config + CLI + glob patterns)
│   ├── ignorefile.rs         # v13: .zacxiomignore support (like .gitignore)
│   ├── scanner.rs            # File discovery engine (exclude-aware)
│   ├── cache.rs              # Cache domain classification
│   ├── ownership.rs          # Package vs user vs system detection
│   ├── risk.rs               # Rule-based risk scoring
│   ├── simulator.rs          # Dry-run + explainable output
│   ├── cleaner.rs            # Safe clean executor (TOCTOU-hardened, atomic copy, checksum)
│   ├── rules.rs              # Immutable safety rules + matches_rules_exclude()
│   ├── pipeline.rs           # Classification pipeline (smart threading, load-aware)
│   ├── snapshot.rs           # XDG-compliant snapshot storage (collision-proof IDs)
│   └── commands/             # CLI command implementations
│       ├── clean.rs          # clean command (confirmation prompts, --yes, --include)
│       ├── config.rs         # v13: config init/show/path subcommands
│       └── ...               # scan, report, explain, status, doctor, etc.
└── tests/
    └── golden/               # Deterministic output tests (help, status, doctor)
```

## 🧠 Core Data Flow

```
                    ┌──────────────┐
                    │   SCANNER    │  file discovery
                    └──────┬───────┘
                           │ Vec<PathBuf>
                    ┌──────▼───────┐
                    │   CACHE      │  classify: browser/system/build/pkg/dev
                    └──────┬───────┘
                           │ Vec<ClassifiedFile>
                    ┌──────▼───────┐
                    │  OWNERSHIP   │  package? user? system? orphan?
                    └──────┬───────┘
                           │ Vec<OwnedFile>
                    ┌──────▼───────┐
                    │    RISK      │  score 0.0–1.0 per rule
                    └──────┬───────┘
                           │ Vec<ScoredFile>
                    ┌──────▼───────┐
                    │  SIMULATOR   │  dry-run: what would happen
                    └──────┬───────┘
                           │ SimulationReport
                    ┌──────▼───────┐
                    │   CLEANER    │  execute safe-only decisions
                    └──────┬───────┘
                           │ CleanReport
                    ┌──────▼───────┐
                    │   REPORT     │  explainable output
                    └──────────────┘
```

## 📐 Data Model

```rust
// The universal pipeline type
struct ClassifiedFile {
    path: PathBuf,
    size: u64,
    cache_domain: CacheDomain,
    ownership: Ownership,
    risk_score: f64,
    risk_reasons: Vec<String>,
    decision: Decision,
}

enum CacheDomain {
    Browser,       // ~/.cache/chromium, ~/.mozilla
    System,        // /var/cache, /tmp
    BuildArtifact, // target/, node_modules, __pycache__
    PackageManager,// /var/cache/apt, /var/cache/pacman
    Developer,     // .gradle, .cargo/registry
    UserData,      // unknown user dirs
    Unknown,
}

enum Ownership {
    Package { pkg_name: String },    // owned by dpkg/rpm
    System,                           // /etc, /usr without package
    User { uid: u32 },               // ~/ owned files
    Orphan,                          // no owning package, not in home
}

enum Decision {
    Safe,        // always ok to clean
    LowRisk,     // clean with --smart
    Moderate,    // require --force
    HighRisk,    // blocked, never auto-clean
    Protected,   // system-critical, can't delete
}
```

## 🔌 Module Boundaries (v1 → v5)

| Module | v1 | v2 | v3 | v4 | v5 |
|--------|----|----|----|----|-----|
| **scanner** | walkdir glob | +depth control | +inode awareness | +snapshot diff | stable |
| **cache** | static paths | +heuristic | +config profiles | +plugin domains | stable API |
| **ownership** | dpkg query | +rpm | +pacman | +nix | stable API |
| **risk** | rule-based | +process aware | +context graph | +policy engine | final v3 |
| **simulator** | mandatory dry-run | +disk estimate | +dependency impact | +rollback preview | stable |
| **cleaner** | safe only | +low-risk | +profile modes | +force gated | final |
| **rules** | immutable rules | +active protection | +health profiles | +user policies | locked |

## 📏 LOC Budget (v5.3.0)

| Module | Target LOC |
|--------|-----------|
| `main.rs` | ~40 |
| `cli.rs` | ~80 |
| `scanner.rs` | ~100 |
| `cache.rs` | ~120 |
| `ownership.rs` | ~80 |
| `risk.rs` | ~100 |
| `simulator.rs` | ~100 |
| `cleaner.rs` | ~80 |
| `rules.rs` | ~60 |
| **Total core** | **~760** |
| **Tests** | ~400 (uncapped) |

## 🧪 Test Philosophy

- Every module has unit tests
- Integration tests use `tests/fixtures/mock_fs/` — never touch real filesystem
- `build.sh check-all` runs: `fmt → clippy → test → audit`
- No feature merges without green `check-all`

## 🔒 Invariants (all versions)

1. Zacxiom never deletes without explicit user intent
2. Every action is logged and explainable
3. `simulate` always runs before `clean` (even `--force`)
4. System-critical paths are hard-coded protected (non-overridable)
5. `--force` requires explicit confirmation prompt
6. No daemon, no background process, no root requirement (unless user chooses)
