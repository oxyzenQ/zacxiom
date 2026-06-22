# Zacxiom v7.0.0 — The Linux Maintenance Companion

**Vision:** Not a cleaner. Not BleachBit. Not an AI gimmick.  
A weekly companion that tells developers what they forgot about.

---

## PHASE 1: STORAGE REALITY AUDIT — Top 50 Consumers

After 3 years on a Linux developer workstation, here's what actually eats storage.

### The "Obvious" Cache (Every Cleaner Finds These)

| # | Consumer | Typical | Safety | Clean Freq | Notes |
|---|---|---|---|---|---|
| 1 | Browser cache (all browsers) | 1-8 GB | ✅ Safe | Weekly | All browsers rebuild on next launch |
| 2 | `~/.cache/` (generic) | 1-5 GB | ✅ Safe | Monthly | Thumbnails, fontconfig, gstreamer |
| 3 | `/tmp/` leftovers | 0.1-2 GB | ✅ Safe | Weekly | tmpfiles.d usually handles this |
| 4 | Package manager cache (apt/pacman/dnf) | 1-10 GB | ✅ Safe | Monthly | `apt clean` / `pacman -Scc` equivalent |

**Subtotal: 3-25 GB** — This is what every tool already handles.

### The "Developer" Cache (Most Tools Miss These)

| # | Consumer | Typical | Safety | Clean Freq | Notes |
|---|---|---|---|---|---|
| 5 | `~/.cargo/registry/` (crate cache) | 2-10 GB | ✅ Safe | Weekly | `cargo build` re-downloads |
| 6 | `~/.cargo/git/` (git checkouts) | 0.5-2 GB | ✅ Safe | Monthly | Re-cloned on next build |
| 7 | `~/project/*/target/` (build output) | 5-50 GB | ✅ Safe | Weekly | `cargo clean` equivalent |
| 8 | `~/.rustup/toolchains/` (old versions) | 2-5 GB | ✅ Safe | Quarterly | Only keep `stable` + `nightly` |
| 9 | `~/.npm/_cacache/` | 1-5 GB | ✅ Safe | Weekly | `npm install` regenerates |
| 10 | `~/.cache/pip/` + `~/.cache/uv/` | 1-5 GB | ✅ Safe | Weekly | `pip install` re-downloads |
| 11 | `~/.cache/pnpm/` + `~/.cache/yarn/` | 1-3 GB | ✅ Safe | Weekly | Package managers regenerate |
| 12 | `~/.gradle/caches/` | 1-5 GB | ✅ Safe | Monthly | Gradle re-downloads |
| 13 | `~/.m2/repository/` (Maven) | 1-5 GB | ✅ Safe | Monthly | Maven re-downloads |
| 14 | `~/.cache/go-build/` | 1-5 GB | ✅ Safe | Monthly | Go rebuilds |
| 15 | `~/project/*/node_modules/` | 1-20 GB | ✅ Safe | Monthly | `npm install` regenerates |
| 16 | `~/project/*/__pycache__/` + `.pyc` | 0.1-1 GB | ✅ Safe | Weekly | Python auto-regenerates |
| 17 | `~/project/*/.venv/` (stale venvs) | 0.5-5 GB | ⚠️ Review | Monthly | Project-specific, might have custom deps |

**Subtotal: 17-126 GB** — This is where the real value lives.

### The "Infrastructure" Cache (Heavy Hitters)

| # | Consumer | Typical | Safety | Clean Freq | Notes |
|---|---|---|---|---|---|
| 18 | Docker overlay2 (`~/.docker/`) | 20-100 GB | ⚠️ Review | Monthly | `docker system prune` but smarter |
| 19 | Docker volumes (orphaned) | 5-50 GB | ⚠️ Review | Quarterly | Unused named volumes |
| 20 | Docker build cache | 5-30 GB | ✅ Safe | Monthly | `docker builder prune` |
| 21 | Podman storage (`~/.local/share/containers/`) | 10-50 GB | ⚠️ Review | Monthly | Same as Docker but Podman |
| 22 | Flatpak runtimes (old versions) | 2-10 GB | ⚠️ Review | Quarterly | `flatpak uninstall --unused` |
| 23 | Snap revisions (old versions) | 2-10 GB | ⚠️ Review | Monthly | Snap keeps 3 revisions by default |
| 24 | systemd journal (`/var/log/journal/`) | 1-10 GB | ⚠️ Review | Monthly | `journalctl --vacuum-size` |
| 25 | coredumps (`/var/lib/systemd/coredump/`) | 0.5-5 GB | ✅ Safe | Monthly | Old crash dumps |

**Subtotal: 45-265 GB** — Docker alone often exceeds everything else combined.

### The "Gaming" Cache

| # | Consumer | Typical | Safety | Clean Freq | Notes |
|---|---|---|---|---|---|
| 26 | Steam shader cache | 2-10 GB | ✅ Safe | Monthly | Regenerated on next launch |
| 27 | Proton compatdata (game prefixes) | 5-30 GB | ⚠️ Review | Per-game | Reinstalled via Steam |
| 28 | Steam downloading cache | 1-20 GB | ✅ Safe | Weekly | Partial downloads |
| 29 | DXVK state cache | 0.5-2 GB | ✅ Safe | Monthly | Regenerated |
| 30 | VKD3D cache | 0.5-2 GB | ✅ Safe | Monthly | Regenerated |
| 31 | Mesa shader cache | 1-5 GB | ✅ Safe | Monthly | Regenerated |
| 32 | Lutris runners (old versions) | 1-5 GB | ⚠️ Review | Quarterly | Only keep active versions |
| 33 | Heroic/Wine prefixes | 2-10 GB | ⚠️ Review | Per-game | Game-specific saves inside |

**Subtotal: 13-84 GB** — Gaming is a massive, overlooked cache category.

### The "Forgotten" Storage (This Is The Differentiator)

| # | Consumer | Typical | Safety | Clean Freq | Notes |
|---|---|---|---|---|---|
| 34 | `~/Downloads/` old files (>90d) | 5-50 GB | ⚠️ Review | Monthly | ISOs, debs, tarballs, PDFs |
| 35 | Old AppImages (`~/Applications/` or `~/Downloads/`) | 1-10 GB | ⚠️ Review | Quarterly | Multiple versions accumulate |
| 36 | Old VM images (`.qcow2`, `.vdi`, `.vmdk`) | 10-100 GB | ⚠️ Review | Quarterly | Abandoned VMs taking 20-50 GB each |
| 37 | Stale git repos (`~/projects/*` untouched >180d) | 1-50 GB | ⚠️ Review | Quarterly | Clone history + build artifacts |
| 38 | AI model checkpoints (`*.pt`, `*.safetensors`, `*.ckpt`) | 10-200 GB | ⚠️ Review | Per-project | Old training checkpoints |
| 39 | AI datasets (`~/datasets/`, `~/data/`) | 5-100 GB | ⚠️ Review | Quarterly | Old versions of datasets |
| 40 | Old Docker images (unused tags) | 5-50 GB | ✅ Safe | Monthly | `docker image prune -a` |
| 41 | `.git/` directories in old clones | 1-20 GB | ⚠️ Review | Per-repo | Full history in abandoned repos |
| 42 | Old `.tar.gz` / `.zip` archives | 1-20 GB | ⚠️ Review | Quarterly | Extracted but archive kept |
| 43 | `~/.local/share/Trash/` (overflowing) | 1-20 GB | ✅ Safe | Weekly | Files user already deleted |
| 44 | Old screenshots (`~/Pictures/Screenshots/`) | 0.5-5 GB | ⚠️ Review | Monthly | Auto-accumulation |
| 45 | Old recordings / screen captures | 5-50 GB | ⚠️ Review | Quarterly | OBS output, meeting recordings |
| 46 | Trading data / market data archives | 5-100 GB | ⚠️ Review | Quarterly | Historical data that's re-downloadable |
| 47 | Old research papers / PDFs in `~/Downloads/` | 0.5-5 GB | ⚠️ Review | Quarterly | Already-read papers |
| 48 | Duplicate files across projects | 1-20 GB | ⚠️ Review | Quarterly | Same dataset/model copied multiple times |
| 49 | Node.js `package-lock.json` bloat | 0.1-1 GB | ✅ Safe | Monthly | Regenerated on `npm install` |
| 50 | Old log files (`*.log` untouched >90d) | 0.5-5 GB | ✅ Safe | Monthly | Application logs, build logs |

**Subtotal (forgotten): 49-976 GB** — This dwarfs everything else.

---

### GRAND TOTAL: A 3-Year Linux Workstation Typically Has

| Category | Conservative | Typical | Heavy |
|---|---|---|---|
| Obvious cache | 3 GB | 10 GB | 25 GB |
| Developer cache | 17 GB | 50 GB | 126 GB |
| Infrastructure | 45 GB | 100 GB | 265 GB |
| Gaming | 13 GB | 30 GB | 84 GB |
| **Forgotten storage** | **49 GB** | **200 GB** | **976 GB** |
| **TOTAL** | **127 GB** | **390 GB** | **1,476 GB** |

**The forgotten category is 2-5× larger than all caches combined.**

No existing tool (BleachBit, ncdu, du, find) systematically finds forgotten storage. They find cache. They don't find abandoned projects, old VMs, stale datasets.

---

## PHASE 2: FORGOTTEN STORAGE DETECTION

### Detection Strategies (Beyond Cache Patterns)

#### Strategy 1: Age-Based Abandonment Detection

```rust
// "This project hasn't been touched in 6 months"
if last_modified > 180.days && is_project_root(path) {
    flag_as("abandoned_project", confidence: 0.7);
    suggest("Archive or delete if no longer needed");
}
```

**Signals:**
- Git repo with no commits in 180 days
- Directory with build artifacts but no recent source changes
- Project directory where all files are >90 days old
- `Cargo.toml` / `package.json` / `pyproject.toml` with old modification time

#### Strategy 2: Size × Age = "Forgotten Heavy"

```rust
// "This 12 GB VM hasn't been powered on in 8 months"
if size > 1.GB && age > 90.days && is_large_blob(path) {
    flag_as("dormant_large_file", confidence: 0.8);
    suggest("Consider archiving or deleting — hasn't been used in {} days", age);
}
```

**Targets:**
- `.qcow2`, `.vdi`, `.vmdk`, `.img` files (>1 GB, >90d old)
- `.tar.gz`, `.tar.xz`, `.zip` files (>100 MB, >180d old)
- `.iso` files (>100 MB, >90d old)
- `.AppImage` files with multiple versions
- `.pt`, `.safetensors`, `.ckpt` model files (>500 MB, >90d old)
- `.csv`, `.parquet`, `.jsonl` dataset files (>100 MB, >180d old)

#### Strategy 3: Duplicate Detection (Content-Aware)

```rust
// "You have the same dataset in 3 different project directories"
if same_size && same_name && different_path {
    flag_as("potential_duplicate", confidence: 0.6);
    suggest("Same file found in multiple locations — keep one copy");
}
```

**Lightweight approach** (no full-content hashing to avoid I/O storm):
- Same filename + same exact size → 80% confidence
- Same filename + same size + same extension → 60% confidence
- Group by name and show user: "dataset_v3.tar.gz found in 3 locations (450 MB each)"

#### Strategy 4: Stale Build Outputs

```rust
// "This target/ directory is 8 GB but the source hasn't changed in 3 months"
if path.ends_with("/target") && source_modified > 90.days {
    flag_as("stale_build", confidence: 0.9);
    suggest("Build output for project last modified {} days ago", age);
}
```

**Targets:**
- `target/` directories where `Cargo.toml` hasn't changed in 90d
- `node_modules/` where `package.json` hasn't changed in 90d
- `build/`, `dist/`, `.next/` directories where source is stale
- `__pycache__/` directories (always safe, but flag large ones)

#### Strategy 5: Version Accumulation

```rust
// "You have 7 versions of the same AppImage"
if is_appimage && multiple_versions_in_same_dir {
    flag_as("version_accumulation", confidence: 0.9);
    suggest("{} old versions found — keep only the latest", count - 1);
}
```

**Targets:**
- Multiple AppImage versions: `AppName-1.0.AppImage`, `AppName-1.1.AppImage`, `AppName-2.0.AppImage`
- Multiple Python versions in `~/.python/` or pyenv
- Multiple Node versions in `~/.nvm/versions/`
- Multiple Rust toolchain dates in `~/.rustup/toolchains/`

#### Strategy 6: Extraction Artifacts

```rust
// "You extracted this archive but kept the archive too"
if extracted_dir.exists() && archive.exists() {
    flag_as("extraction_artifact", confidence: 0.7);
    suggest("Archive {} can be deleted — contents already extracted", archive);
}
```

**Pattern:** `data.tar.gz` (5 GB) + `data/` directory (5 GB extracted) → 5 GB duplicate

---

### Classification: Beyond `CacheDomain`

The current system has 7 domains. For forgotten storage, we need new dimensions:

```rust
enum StorageCategory {
    // Existing
    Cache,
    BuildArtifact,
    PackageManager,
    
    // NEW — Forgotten Storage
    AbandonedProject,      // Stale git repos, old projects
    DormantLargeFile,      // Old VMs, ISOs, datasets untouched >90d
    VersionAccumulation,   // Multiple old versions
    ExtractionDuplicate,   // Archive + extracted content
    StaleDownload,         // ~/Downloads/ files >90d old
    OldLog,                // *.log files untouched >90d
    DuplicateFile,         // Same file in multiple locations
    MediaAccumulation,     // Screenshots, recordings, camera dumps
    ResearchArtifact,      // Old papers, datasets, model checkpoints
}
```

---

## PHASE 3: EXPLAINABILITY FRAMEWORK

Every recommendation must answer 5 questions. Here's the framework:

### The "5 Questions" Rule

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  📦 Cargo Registry Cache                        │
│                                                 │
│  WHAT IS THIS?                                  │
│  Downloaded Rust crate files cached by Cargo.   │
│  These are .crate files from crates.io.         │
│                                                 │
│  WHY IS IT SAFE?                                │
│  ★★★★★ Maximum Safety                           │
│  Cargo automatically re-downloads any missing   │
│  crates on the next `cargo build`. You will     │
│  never lose code or data.                       │
│                                                 │
│  WHAT HAPPENS IF DELETED?                       │
│  Next `cargo build` will take 2-5 minutes       │
│  longer as crates re-download. That's it.       │
│                                                 │
│  HOW MUCH SPACE?                                │
│  2.3 GB across 847 files                        │
│                                                 │
│  CAN IT BE RESTORED?                            │
│  Yes — just run `cargo build` again.            │
│  Or restore from snapshot (created before       │
│  cleaning).                                     │
│                                                 │
└─────────────────────────────────────────────────┘
```

### Safety Tier System

| Tier | Name | Icon | Meaning | Examples |
|---|---|---|---|---|
| 5 | **Maximum Safety** | ★★★★★ | Fully regenerable, zero data loss | Browser cache, Cargo registry, pip cache |
| 4 | **High Safety** | ★★★★☆ | Regenerable but takes time | Docker build cache, node_modules, target/ |
| 3 | **Review Recommended** | ★★★☆☆ | Probably safe, but check first | Old downloads, stale projects, old logs |
| 2 | **Caution** | ★★☆☆☆ | May contain useful data | Proton prefixes, old VM images, AI checkpoints |
| 1 | **Manual Review Required** | ★☆☆☆☆ | Could be important | Active project data, keyrings, configs |
| 0 | **Protected** | ⛔ | Never deletable | /etc, /boot, SSH keys, GPG keys |

### Decision Preview

Before cleaning, show exactly what will happen:

```
$ zacxiom clean --dry-run

═══ PREVIEW: Smart Clean ═══

SAFE TO CLEAN (★★★★★)
  ✅ Cargo Registry        2.3 GB   847 files   (re-downloads automatically)
  ✅ npm Cache             1.2 GB   312 files   (npm install regenerates)
  ✅ Browser Cache         0.8 GB   2,401 files (browsers rebuild cache)
  ─────────────────────────────────
  Subtotal:                4.3 GB   3,560 files

REVIEW RECOMMENDED (★★★☆☆)
  ⚠️  Old Downloads         5.1 GB   23 files    (ISOs, debs from 2025)
  ⚠️  Stale Projects        8.2 GB   4 projects  (no commits in 6+ months)
  ─────────────────────────────────
  Subtotal:               13.3 GB   27 items

WILL NOT TOUCH
  🛡️  SSH Keys             12 KB    2 files     (H2 protected)
  🛡️  Active Projects      2.1 GB   3 projects  (modified this week)
  ─────────────────────────────────
  Subtotal:                2.1 GB   5 items

═══════════════════════════════════════
  Total reclaimable:      17.6 GB
  With --smart:            4.3 GB  (safe only)
  With --review:          17.6 GB  (safe + review)
═══════════════════════════════════════

Run: zacxiom clean --smart     (safe only, 4.3 GB)
Run: zacxiom clean --review    (include review items, 17.6 GB)
```

---

## PHASE 4: WEEKLY WORKFLOW

### `zacxiom weekly` — The Habit Command

```
$ zacxiom weekly

╔══════════════════════════════════════════════════════════════╗
║                 WEEKLY STORAGE HEALTH                       ║
║                 Monday, June 22, 2026                       ║
╚══════════════════════════════════════════════════════════════╝

  📊 OVERVIEW
  ─────────────────────────────────────────────────────────────
  Total scanned:     847,231 files (312.4 GB)
  Reclaimable:        24.7 GB  │██████████████████████████████████│
  Protected:           1.2 GB  │█                                   │
  Active (in use):   286.5 GB  │████████████████████████████████████│

  📈 TREND (4 weeks)
  ─────────────────────────────────────────────────────────────
  Week 1:  ████████████ 18.2 GB reclaimed
  Week 2:  ██████        9.7 GB reclaimed
  Week 3:  ████          5.1 GB reclaimed
  Week 4:  ██████████   14.8 GB new cache accumulated

  🔍 TOP CONSUMERS
  ─────────────────────────────────────────────────────────────
  ★★★★★ Cargo Registry                  2.3 GB  (safe)
  ★★★★☆ Docker Overlay                 12.4 GB  (safe, takes time)
  ★★★★★ Browser Cache                   0.8 GB  (safe)
  ★★★☆☆ Old Downloads                   5.1 GB  (review)
  ★★★☆☆ Stale Project: `old-research`   4.2 GB  (no commits in 8 months)

  🆕 NEW THIS WEEK
  ─────────────────────────────────────────────────────────────
  • Stale project detected: `~/projects/defi-bot-2024`
    Last commit: November 2025 (7 months ago)
    Size: 3.8 GB (including 2.1 GB target/)
    Suggestion: Archive to external drive or delete

  • Large dormant file: `~/Downloads/ubuntu-24.04.iso`
    Size: 5.7 GB, last accessed: February 2026
    Suggestion: Delete — you already installed this

  💡 RECOMMENDATION
  ─────────────────────────────────────────────────────────────
  Run: zacxiom clean --smart    → reclaim 15.5 GB safely
  Run: zacxiom clean --review   → reclaim 24.7 GB (needs review)
  Run: zacxiom scan --new       → see only new findings

  ─────────────────────────────────────────────────────────────
  Last cleaned: 7 days ago (freed 18.2 GB)
  Total freed all-time: 47.3 GB across 4 cleans
```

### Why This Works — The Psychology

1. **Immediate value signal** — "24.7 GB reclaimable" catches attention
2. **Frictionless** — One command, 5 seconds, zero thinking
3. **New findings** — "🆕 NEW THIS WEEK" creates curiosity to check
4. **Trust building** — Stars, safety tiers, explanations build confidence
5. **Progress visible** — Trend chart shows impact over time
6. **Low risk default** — `--smart` only cleans ★★★★★ items
7. **Discovery** — Finds things the user forgot about (dormant projects, old ISOs)

---

## PHASE 5: MASTERCLASS ROADMAP

### Scoring System
- **Utility (U):** How useful is this to a real developer? (1-10)
- **Trust (T):** How much does this increase confidence? (1-10)
- **Storage (S):** How much additional storage does this unlock? (1-10)
- **Cost (C):** Implementation effort (1-10, lower = easier)

### Priority Matrix

| # | Feature | U | T | S | C | Score | Version |
|---|---|---|---|---|---|---|---|
| 1 | **Expand default scan roots** (cargo, npm, docker, steam, rustup) | 10 | 8 | 10 | 1 | **29** | v6.1.0 |
| 2 | **10 new domain patterns** (Steam, Docker, yay, uv, Flatpak, etc.) | 10 | 8 | 10 | 2 | **28** | v6.1.0 |
| 3 | **Safety tier system (★★★★★)** | 8 | 10 | 0 | 2 | **20** | v6.1.0 |
| 4 | **`--dry-run` preview mode** | 9 | 10 | 0 | 2 | **21** | v6.2.0 |
| 5 | **`--domains` filtering** (clean only what you choose) | 9 | 9 | 0 | 3 | **18** | v6.2.0 |
| 6 | **Age-based abandonment detection** (>180d projects) | 10 | 7 | 9 | 4 | **22** | v6.2.0 |
| 7 | **Dormant large file detection** (>1GB, >90d untouched) | 10 | 7 | 10 | 3 | **24** | v6.2.0 |
| 8 | **5-Question explainability cards** | 7 | 10 | 0 | 3 | **14** | v6.2.0 |
| 9 | **Duplicate file detection** (same name+size) | 8 | 6 | 8 | 5 | **17** | v7.0.0 |
| 10 | **Version accumulation detection** (multiple AppImages/toolchains) | 7 | 7 | 7 | 4 | **17** | v7.0.0 |
| 11 | **`zacxiom weekly` command** (habit-forming report) | 9 | 8 | 0 | 6 | **11** | v7.0.0 |
| 12 | **Storage trend tracking** (week-over-week charts) | 7 | 7 | 0 | 5 | **9** | v7.0.0 |
| 13 | **Extraction artifact detection** (archive + extracted dir) | 7 | 6 | 6 | 4 | **15** | v7.0.0 |
| 14 | **systemd timer integration** (auto weekly scan) | 6 | 5 | 0 | 3 | **8** | v7.0.0 |
| 15 | **Interactive mode** (TUI domain selection) | 6 | 8 | 0 | 8 | **6** | v8.0.0 |

---

### Roadmap

```
v6.1.0 ─ SCAN WHAT MATTERS ──────────────────────────────────
  ✅ Expand default scan roots (cargo, npm, docker, steam, rustup)
  ✅ 10 new domain patterns (Steam, Proton, Docker, yay, paru, uv, Flatpak, Snap, Trash, vkd3d)
  ✅ Safety tier stars (★★★★★)
  
  Effect: Default scan finds 5-10× more reclaimable storage
  
v6.2.0 ─ BUILD TRUST ────────────────────────────────────────
  ✅ --dry-run preview mode
  ✅ --domains cargo,docker,browser filtering
  ✅ 5-Question explainability cards
  ✅ Age-based abandonment detection (projects untouched >180d)
  ✅ Dormant large file detection (>1GB, >90d old)
  
  Effect: Users confidently run `zacxiom clean` without fear
          "Forgotten storage" category finds 50-200 GB of overlooked files

v7.0.0 ─ HABIT FORMATION ────────────────────────────────────
  ✅ `zacxiom weekly` command with beautiful report
  ✅ Storage trend tracking (growth charts, all-time stats)
  ✅ Duplicate detection (same file in multiple places)
  ✅ Version accumulation (old AppImages, toolchains, SDKs)
  ✅ Extraction artifact detection (archive + extracted = double)
  ✅ systemd timer for auto weekly scan
  
  Effect: Zacxiom becomes a weekly habit, like checking email
          "The Linux maintenance companion developers actually use"
```

---

## THE CORE INSIGHT

Zacxiom's real competition isn't BleachBit. It's the developer's own procrastination:

> "I know there's probably 50 GB of junk on this machine but I don't have 30 minutes to investigate what's safe to delete."

Zacxiom wins when it answers that question in 5 seconds with high confidence.

The cache detection is table stakes. The **forgotten storage detection** is the differentiator that makes users say:

> "I didn't even remember I had that old VM. Thanks for finding it."

That's when Zacxiom becomes indispensable.
