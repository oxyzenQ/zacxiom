# 🧱 ZACXIOM ARCHITECTURE v1.0.0 — v5.0.0

> **Constraint:** Core engine <1k LOC. Plugins unlimited.
> **Law:** If a module doesn't serve safety, explainability, or awareness — it doesn't exist.

---

## 📦 Crate Layout

```
zacxiom/
├── Cargo.toml
├── build.sh                  # build + check-all
├── ARCHITECTURE.md
├── RULES.md
├── src/
│   ├── main.rs               # Entry point (<60 LOC)
│   ├── cli.rs                # clap derive (scan/report/simulate/clean)
│   ├── scanner.rs            # File discovery engine
│   ├── cache.rs              # Cache domain classification
│   ├── ownership.rs          # Package vs user vs system detection
│   ├── risk.rs               # Rule-based risk scoring
│   ├── simulator.rs          # Dry-run + explainable output
│   ├── cleaner.rs            # Safe clean executor
│   └── rules.rs              # Immutable safety rules
└── tests/
    ├── integration/
    │   ├── scan_test.rs
    │   ├── simulate_test.rs
    │   └── clean_test.rs
    └── fixtures/
        └── mock_fs/           # Controlled test filesystem
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

## 📏 LOC Budget (v1.0.0)

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
