# Zacxiom Masterclass Usefulness Audit

**Date:** 2026-06-22  
**Assessment:** The engine is solid. The default behavior doesn't deliver on the promise yet.

---

## PHASE 1: REAL USER VALUE ANALYSIS

### Why would a Linux user run Zacxiom every week?

The Linux developer who has used the same workstation for 3 years faces 60 real pain points:

### Top 20 Storage Pain Points

| # | Pain Point | Typical Size | Frequency | Frustration |
|---|---|---|---|---|
| 1 | Cargo registry cache grows forever | 2-10 GB | Weekly | HIGH |
| 2 | Rust target/ directories accumulate | 5-50 GB | Daily | HIGH |
| 3 | npm cache (`~/.npm/_cacache`) | 1-5 GB | Weekly | MEDIUM |
| 4 | Docker images/overlay/volumes | 20-100 GB | Monthly | HIGH |
| 5 | Steam shader caches | 2-10 GB | Monthly | MEDIUM |
| 6 | Proton compatdata prefixes | 5-30 GB | Monthly | HIGH |
| 7 | pip cache (`~/.cache/pip`) | 0.5-3 GB | Weekly | LOW |
| 8 | Browser caches (Firefox/Chromium) | 1-5 GB | Weekly | MEDIUM |
| 9 | node_modules across projects | 1-20 GB | Monthly | HIGH |
| 10 | `~/.rustup` old toolchains | 2-5 GB | Quarterly | MEDIUM |
| 11 | `~/.gradle/caches` | 1-5 GB | Monthly | MEDIUM |
| 12 | `~/.m2/repository` (Maven) | 1-5 GB | Monthly | MEDIUM |
| 13 | `~/.cache/yarn` | 0.5-2 GB | Monthly | LOW |
| 14 | `~/.cache/pnpm` | 0.5-2 GB | Monthly | LOW |
| 15 | `~/.cache/go-build` | 1-5 GB | Monthly | MEDIUM |
| 16 | `~/.cache/JetBrains` (IDE caches) | 2-10 GB | Monthly | HIGH |
| 17 | `~/.cache/thumbnails` | 0.1-1 GB | Monthly | LOW |
| 18 | Desktop Trash | 1-10 GB | Monthly | LOW |
| 19 | `~/.cache/vscode-*` (VS Code) | 0.5-3 GB | Monthly | MEDIUM |
| 20 | Flatpak runtime caches | 1-10 GB | Monthly | MEDIUM |

### Top 20 Cleanup Pain Points

| # | Pain Point | Why It Hurts |
|---|---|---|
| 1 | "Is this safe to delete?" | No confidence in what's cache vs data |
| 2 | "Where ARE my caches?" | Rust/Node/Docker caches scattered everywhere |
| 3 | "How much can I reclaim?" | `du` shows size, not reclaimability |
| 4 | "Will deleting this break something?" | Fear of breaking running programs |
| 5 | "I already cleaned this last month" | No memory of previous cleanups |
| 6 | "Which of these 5000 files matter?" | Analysis paralysis from large scans |
| 7 | "Is this Docker cache or container data?" | Hard to distinguish |
| 8 | "Can I clean this without restarting?" | Process-locked files risk |
| 9 | "I need sudo for this" | Permission friction |
| 10 | "I accidentally deleted something important" | No undo — permanent loss |
| 11 | "These are ALL node_modules..." | Known safe but tedious to find |
| 12 | "My SSD is full and I don't know why" | No summary of what's eating space |
| 13 | "Do I need to keep old Rust toolchains?" | Uncertainty about dependencies |
| 14 | "Proton prefixes are huge, which can I delete?" | Game-specific knowledge required |
| 15 | "I don't remember what I downloaded last year" | Downloads folder rot |
| 16 | "Flatpak keeps growing, what's in there?" | Opaque storage |
| 17 | "Snap revisions stacking up" | Hidden automatic snapshots |
| 18 | "journalctl logs are eating my /var" | System logs invisible to user |
| 19 | "Pacman/yay/paru cache piling up" | Arch-specific knowledge needed |
| 20 | "I'm scared of rm -rf" | Fear of catastrophic mistakes |

### Top 20 Maintenance Pain Points

| # | Pain Point |
|---|---|
| 1 | No weekly cleanup habit |
| 2 | Forgetting to clean for months |
| 3 | Only cleaning when disk is full |
| 4 | Emergency cleanup = risky cleanup |
| 5 | No idea what "normal" disk usage looks like |
| 6 | Can't track storage growth over time |
| 7 | Different tools for different caches |
| 8 | Manual `find` commands are error-prone |
| 9 | `ncdu` is great but no safety guidance |
| 10 | `bleachbit` is too aggressive or too conservative |
| 11 | No "dry run" that shows actual reclaimable GB |
| 12 | Can't schedule automatic safe cleans |
| 13 | No confidence score before deleting |
| 14 | Can't preview what would be deleted |
| 15 | No history of what was cleaned when |
| 16 | Cross-distro differences in cache locations |
| 17 | New tools add new cache locations constantly |
| 18 | WSL/container cache proliferation |
| 19 | Multi-user systems have scattered caches |
| 20 | "I just want one command that handles everything safely" |

---

## PHASE 2: COVERAGE GAP ANALYSIS

### Developer Coverage

| Tool | Path | Current Domain | Should Be | Storage Impact | Fix Complexity |
|---|---|---|---|---|---|
| Cargo registry | `~/.cargo/registry/cache` | Developer ✅ | Developer ✅ | 2-10 GB | None |
| Cargo registry src | `~/.cargo/registry/src` | Developer ✅ | Developer ✅ | 1-5 GB | None |
| Cargo git | `~/.cargo/git` | Developer ✅ | Developer ✅ | 0.5-2 GB | None |
| Rust target/ | `~/project/target` | BuildArtifact ✅ | BuildArtifact ✅ | 5-50 GB | None |
| Rustup toolchains | `~/.rustup/toolchains/*/tmp` | **Unknown ❌** | Developer | 2-5 GB | **Easy** |
| npm cache | `~/.npm/_cacache` | Developer ✅ | Developer ✅ | 1-5 GB | None |
| pnpm store | `~/.cache/pnpm` | Developer ✅ | Developer ✅ | 0.5-2 GB | None |
| yarn cache | `~/.cache/yarn` | Developer ✅ | Developer ✅ | 0.5-2 GB | None |
| pip cache | `~/.cache/pip` | Developer ✅ | Developer ✅ | 0.5-3 GB | None |
| uv cache | `~/.cache/uv` | **UserData ❌** | Developer | 0.5-2 GB | **Easy** |
| venv/virtualenv | `~/project/.venv` | **Unknown ❌** | BuildArtifact | 0.1-2 GB | **Medium** |
| Docker overlay | `~/.docker/overlay2` | **Unknown ❌** | Developer | 20-100 GB | **Easy** |
| Docker build | `/var/lib/docker` | **Unknown ❌** | System | 20-100 GB | **Easy** |
| Podman storage | `~/.local/share/containers` | **Unknown ❌** | System | 10-50 GB | **Easy** |
| Gradle | `~/.gradle/caches` | BuildArtifact ✅ | BuildArtifact ✅ | 1-5 GB | None |
| Maven | `~/.m2/repository` | Developer ✅ | Developer ✅ | 1-5 GB | None |
| Go build | `~/.cache/go-build` | UserData | UserData | 1-5 GB | Could be Developer |
| JetBrains IDE | `~/.cache/JetBrains` | UserData | UserData | 2-10 GB | Could add IDE domain |
| VS Code | `~/.cache/vscode-*` | UserData | UserData | 0.5-3 GB | Could add IDE domain |
| Node modules | `~/project/node_modules` | BuildArtifact ✅ | BuildArtifact ✅ | 1-20 GB | None |

### Gaming Coverage

| Cache | Path | Current Domain | Should Be | Storage Impact | Fix Complexity |
|---|---|---|---|---|---|
| Steam shaders | `~/.steam/steam/steamapps/shadercache` | **Unknown ❌** | System/Gaming | 2-10 GB | **Easy** |
| Proton compatdata | `~/.steam/steam/steamapps/compatdata` | **Unknown ❌** | UserData/Gaming | 5-30 GB | **Easy** |
| Steam downloads | `~/.steam/steam/steamapps/downloading` | **Unknown ❌** | System | 1-20 GB | **Easy** |
| Steam runtime | `~/.local/share/Steam` | **Unknown ❌** | System | 1-5 GB | **Easy** |
| DXVK cache | `~/.cache/dxvk-cache` | UserData ✅ | UserData | 0.5-2 GB | None |
| VKD3D cache | `~/.cache/vkd3d` | **UserData ❌** | UserData (acceptable) | 0.5-1 GB | **Easy** |
| Mesa shaders | `~/.cache/mesa_shader_cache` | System ✅ | System | 0.5-5 GB | None |
| Lutris runners | `~/.local/share/lutris/runners` | **Unknown ❌** | System | 1-5 GB | **Easy** |
| Heroic cache | `~/.config/heroic` | **Unknown ❌** | System | 1-3 GB | **Easy** |
| Wine prefixes | `~/.wine` | **Unknown ❌** | UserData | 1-5 GB | **Easy** |

### Desktop Coverage

| Cache | Path | Current Domain | Should Be | Storage Impact | Fix Complexity |
|---|---|---|---|---|---|
| Firefox cache | `~/.cache/mozilla` | Browser ✅ | Browser ✅ | 0.5-3 GB | None |
| Chromium cache | `~/.cache/chromium` | Browser ✅ | Browser ✅ | 0.5-3 GB | None |
| Chrome cache | `~/.cache/google-chrome` | Browser ✅ | Browser ✅ | 0.5-3 GB | None |
| Brave cache | `~/.cache/brave` | Browser ✅ | Browser ✅ | 0.3-2 GB | None |
| Edge cache | `~/.cache/edge` | Browser ✅ | Browser ✅ | 0.3-2 GB | None |
| Desktop Trash | `~/.local/share/Trash` | **Unknown ❌** | UserData | 1-10 GB | **Easy** |
| Downloads | `~/Downloads` | Unknown | Unknown (correct) | 1-50 GB | None (should warn) |
| Flatpak cache | `~/.var/app/*/cache` | UserData | UserData (acceptable) | 1-10 GB | Could add Flatpak |
| Snap cache | `~/snap/*/common/.cache` | UserData | UserData | 1-5 GB | Could add Snap |
| Thumbnails | `~/.cache/thumbnails` | UserData ✅ | UserData ✅ | 0.1-1 GB | None |
| Fontconfig | `~/.cache/fontconfig` | UserData ✅ | UserData ✅ | 0.05-0.1 GB | None |

### Arch Linux Coverage (System)

| Cache | Path | Current Domain | Storage Impact | Fix Complexity |
|---|---|---|---|---|
| pacman cache | `/var/cache/pacman/pkg` | PackageManager ✅ | 1-10 GB | None |
| yay cache | `~/.cache/yay` | **UserData ❌** | 1-5 GB | **Easy** |
| paru cache | `~/.cache/paru` | **UserData ❌** | 1-5 GB | **Easy** |
| journalctl | `/var/log/journal` | **Unknown ❌** | 0.5-5 GB | **Easy** |

---

### CRITICAL FINDING: Default Scan Misses the Biggest Caches

**Zacxiom's `default_scan_roots()` only scans:**
```
~/.cache/
~/.local/share/Trash/
/var/cache/
/tmp/
```

**It does NOT scan by default:**
```
~/.cargo/          ← 2-10 GB Rust caches
~/.rustup/          ← 2-5 GB old toolchains
~/.npm/             ← 1-5 GB npm cache
~/.docker/          ← 20-100 GB Docker overlay
~/.steam/           ← 10-30 GB Steam everything
~/.local/share/Steam/ ← 1-5 GB Steam runtime
~/.gradle/          ← 1-5 GB Gradle
~/.m2/              ← 1-5 GB Maven
~/project/*/target/ ← 5-50 GB per project
~/project/*/node_modules/ ← 1-10 GB per project
```

**This is the #1 usefulness blocker.** The default scan misses 80% of reclaimable storage because it doesn't look in the right places.

---

## PHASE 3: TRUST AUDIT

### What prevents users from pressing `zacxiom clean` without fear?

| Trust Barrier | Severity | Current State |
|---|---|---|
| **LowRisk ≠ Safe** | HIGH | 520/559 files classified as LowRisk — none as Safe. Users see "risk" everywhere. |
| **No preview mode** | HIGH | Can't see "what WOULD be deleted with --smart" vs "--force" |
| **No confidence score** | MEDIUM | Risk score 0.0-1.0 is opaque to users |
| **Explanations are technical** | MEDIUM | "fully regenerable cache" vs "This is a Cargo crate cache. cargo build will re-download it." |
| **No "trusted" marking** | MEDIUM | Memory module learns but doesn't show user what it learned |
| **All-or-nothing decisions** | MEDIUM | No per-domain selection ("clean only cargo caches, leave browser" |
| **"Moderate" is scary** | LOW | Users see Moderate and hesitate — even when it's just an old download |
| **No undo preview** | LOW | Snapshot exists but no "this would create a snapshot of X MB" |

### Trust Recommendations

| # | Improvement | Impact | Cost |
|---|---|---|---|
| 1 | **Preview mode**: `zacxiom clean --dry-run` shows what WOULD happen | 10 | 2 |
| 2 | **Confidence tiers**: Replace "LowRisk" with "Probably Safe" language | 8 | 2 |
| 3 | **Domain-specific cleaning**: `zacxiom clean --domains cargo,browser` | 9 | 3 |
| 4 | **Human explanations**: "This cache will be rebuilt next time you run `cargo build`" | 7 | 3 |
| 5 | **Interactive mode**: Show domains → user picks → confirm → clean | 8 | 5 |

---

## PHASE 4: HABIT FORMATION

### Workflows That Make Users Come Back

#### Developer Monday Morning
```bash
$ zacxiom scan
═══ Developer Cache Report ═══
  Cargo Registry   2.3 GB  │████████████    │ SAFE — cargo build regenerates
  Rust Build Cache 4.1 GB  │████████████████│ SAFE — target/ is build output
  npm Cache        1.2 GB  │██████          │ SAFE — npm install regenerates
  Browser Cache    0.8 GB  │████            │ SAFE — browsers rebuild cache
  Docker Overlay   12.4 GB │████████████████████████████████████████████│ SAFE — docker build regenerates
  ─────────────────────────
  Reclaimable: 20.8 GB (safe to clean)
  
$ zacxiom clean --smart
  ✅ Freed 18.2 GB (12,431 files)
  💾 Snapshot saved: zacxiom-2026-06-22.snap
```

#### Gaming Friday Night
```bash
$ zacxiom scan
═══ Gaming Cache Report ═══
  Steam Shader Cache  3.2 GB │████████████    │ SAFE — regenerates on next launch
  Proton Compatdata   8.7 GB │████████████████████████████████████│ SAFE — reinstalls via Steam
  DXVK Cache          0.9 GB │███             │ SAFE — regenerates
  Mesa Shader Cache   1.4 GB │█████           │ SAFE — regenerates
  ─────────────────────────
  Reclaimable: 14.2 GB
  
$ zacxiom clean --smart
  ✅ Freed 14.2 GB
```

#### Monthly Maintenance Saturday
```bash
$ zacxiom status
═══ System Overview ═══
  Total scanned:        156,432 files (89.3 GB)
  Reclaimable (safe):   42.1 GB  │████████████████████████████████████████████│
  Needs review:          8.2 GB  │████████                                    │
  Protected:             0 files
  ───────────────────────
  Last cleaned: 28 days ago (freed 18.2 GB)
  Storage trend: 📈 +3.2 GB since last clean
```

### Why These Work

1. **Shows value upfront** — "20.8 GB reclaimable" is compelling
2. **No fear** — "SAFE — cargo build regenerates" builds trust
3. **Habit loop** — Same time each week, positive experience
4. **Progress visible** — Storage trend shows impact over time
5. **Low friction** — One command, no research needed

---

## PHASE 5: COMPETITIVE ANALYSIS

| Capability | Zacxiom | ncdu | du | find+rm | bleachbit |
|---|---|---|---|---|---|
| Disk usage view | ✅ | ✅✅ | ✅ | ❌ | ❌ |
| Interactive browse | ❌ | ✅✅ | ❌ | ❌ | ❌ |
| Domain classification | ✅ | ❌ | ❌ | ❌ | ✅✅ |
| Risk scoring | ✅ | ❌ | ❌ | ❌ | ❌ |
| Process-aware safety | ✅ | ❌ | ❌ | ❌ | ❌ |
| Adaptive learning | ✅ | ❌ | ❌ | ❌ | ❌ |
| Undo/snapshot | ✅ | ❌ | ❌ | ❌ | ❌ |
| Human explanations | ✅ | ❌ | ❌ | ❌ | ❌ |
| Domain coverage | 8 | N/A | N/A | N/A | 1000+ |
| Speed | Fast | Fast | Fast | Fast | Slow |
| Scriptable | ✅ JSON | ❌ | ✅ | ✅ | ❌ |
| Zero deps (no GUI) | ✅ | ✅ | ✅ | ✅ | Requires GTK |

### What Zacxiom should NEVER become:
- A GUI tool (stay CLI — that's the niche)
- A system cleaner that touches /etc or /boot
- A "registry cleaner" (that's Windows nonsense)
- A replacement for `ncdu` (different use case — ncdu is exploration, Zacxiom is decision)

### What Zacxiom does BETTER than anyone:
1. **Explains decisions** — no other tool tells you WHY a file is safe or risky
2. **Learns from you** — remembers what you've safely cleaned before
3. **Process-aware** — won't touch files open by running programs
4. **Undo support** — only tool with snapshot-based recovery

### What Zacxiom does WORSE:
1. **Default scan coverage** — misses the biggest caches (fixed by expanding roots)
2. **Domain recognition** — 89% vs bleachbit's 1000+ patterns
3. **No interactive browse** — ncdu lets you explore; Zacxiom just reports

---

## PHASE 6: MASTERCLASS ROADMAP

### Scoring: Impact (1-10), Cost (1-10, lower=easier), Trust (1-10), Storage (1-10)

| # | Improvement | Impact | Cost | Trust | Storage | Score |
|---|---|---|---|---|---|---|
| 1 | **Expand default scan roots** — add ~/.cargo, ~/.npm, ~/.rustup, ~/.docker, ~/.steam | 10 | 1 | 9 | 10 | **30** |
| 2 | **Add Steam/Proton/Docker patterns** — 10 new domain classifications | 10 | 2 | 8 | 10 | **28** |
| 3 | **Preview/dry-run mode** — `zacxiom clean --dry-run` | 9 | 2 | 10 | 0 | **21** |
| 4 | **Domain-specific cleaning** — `zacxiom clean --domains cargo,browser` | 9 | 3 | 9 | 0 | **18** |
| 5 | **Human-readable explanations** — "cargo build regenerates this" | 7 | 2 | 9 | 0 | **18** |
| 6 | **Default depth increase** — scan deeper to find nested caches | 8 | 1 | 0 | 8 | **17** |
| 7 | **Confidence tiers** — "Probably Safe" / "Needs Review" / "Do Not Delete" | 8 | 2 | 10 | 0 | **16** |
| 8 | **Storage trend tracking** — "3.2 GB grown since last week" | 6 | 4 | 0 | 0 | **10** |
| 9 | **Interactive mode** — select domains, confirm, clean | 7 | 5 | 8 | 0 | **10** |
| 10 | **Weekly reminder** — cron/systemd timer integration | 5 | 3 | 0 | 0 | **8** |

### Recommended Roadmap

#### v6.1.0 — Scan What Matters
- ⭐ **Expand default scan roots** (10 lines of code, maximum impact)
- ⭐ **Add 10 domain patterns** (Steam, Proton, Docker, yay, paru, uv, vkd3d, Flatpak, Snap, Trash)
- ⭐ **Increase default depth** (or make depth apply to directory count, not absolute)
- **Effect:** Default scan finds 80% more reclaimable storage

#### v6.2.0 — Build Trust
- ⭐ **Dry-run mode** — `zacxiom clean --dry-run`
- ⭐ **Domain-specific cleaning** — `--domains cargo,browser`
- ⭐ **Human explanations** — tool-specific regeneration notes
- **Effect:** Users confidently run `zacxiom clean` instead of hesitating

#### v7.0.0 — Masterclass
- Interactive domain selection
- Storage trend tracking
- systemd timer integration for auto-scan
- Weekly report summaries

---

## THE ANSWER

**"What reason does a user have to open Zacxiom next week?"**

After one run:

```
$ zacxiom scan
═══ You can reclaim 20.8 GB right now ═══
  ✅ Cargo Registry    2.3 GB  (safe — cargo build regenerates)
  ✅ Rust Build Cache  4.1 GB  (safe — build output)
  ✅ npm Cache         1.2 GB  (safe — npm install regenerates)
  ✅ Docker Overlay   12.4 GB  (safe — docker build regenerates)
  ✅ Browser Cache     0.8 GB  (safe — browsers rebuild cache)
  
  Run: zacxiom clean --smart
```

**The answer:** Because in 5 seconds, Zacxiom tells the user something useful they didn't know — exactly how much storage they can safely reclaim, where it is, and why it's safe. No other tool does this.

The user comes back next week because:
- They reclaimed 18 GB last time and felt good
- New caches have accumulated
- It takes 5 seconds and zero brainpower
- They trust it more each time ("it's never deleted anything important")

**The #1 fix right now: expand default scan roots.** Without that, Zacxiom is scanning the wrong places and the value proposition collapses.
