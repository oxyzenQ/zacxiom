# Zacxiom Product Strategy: From Cleaner to Storage Intelligence Platform

**Vision Shift:** Cache cleaner → Linux Storage Intelligence Platform  
**Core Question:** "Why is my SSD down to 40 GB when I don't remember saving anything?"  
**North Star:** A tool Linux users install once and never uninstall.

---

## 1. HABIT FORMATION

The difference between "tool I tried" and "tool I depend on" is habit.

### Weekly Habit Loop

```
Monday morning → zacxiom weekly → "24 GB reclaimable" → zacxiom clean safe → done in 10 seconds
```

This must feel as natural as checking email. The key: **the output must contain something new every time** or users stop checking.

### Three Habit Commands

#### `zacxiom weekly` — The Gateway Drug

```
$ zacxiom weekly

╔══════════════════════════════════════════════════════════╗
║           WEEKLY STORAGE HEALTH                         ║
║           June 22, 2026                                 ║
╚══════════════════════════════════════════════════════════╝

  📊 HEADLINE
  ─────────────────────────────────────────────────────────
  24.8 GB reclaimable this week → 4 new findings

  🔍 WHAT'S NEW SINCE LAST WEEK
  ─────────────────────────────────────────────────────────
  🆕 Old Downloads              +5.1 GB  (3 ISOs from 2025)
  🆕 Docker layers (orphaned)   +2.3 GB  (3 unused images)
  📈 Cargo Registry             +0.8 GB  (growth: +40% this month)

  ⚡ QUICK WIN (★★★★★ safe)
  ─────────────────────────────────────────────────────────
  Cargo Registry        2.7 GB  │███████████│  Clean now
  Browser Cache         0.9 GB  │████        │  Clean now
  npm Cache             1.3 GB  │██████      │  Clean now
  ─────────────────────────────────
  Clean all safe items:  4.9 GB  in 3 seconds

  👀 NEEDS YOUR ATTENTION
  ─────────────────────────────────────────────────────────
  ⚠️  Old Downloads        5.1 GB  (3 files, >180 days old)
  ⚠️  Stale project        4.2 GB  (no commits in 8 months)
  ⚠️  Dormant VM image     4.6 GB  (ubuntu-dev.qcow2, 11 months untouched)

  💡 SMART RECOMMENDATION
  ─────────────────────────────────────────────────────────
  Run: zacxiom clean safe     → 4.9 GB (zero risk)
  Run: zacxiom scan review    → see 13.9 GB review items

  ─────────────────────────────────────────────────────────
  All-time: 247 GB reclaimed across 31 cleans
  Streak: 8 weeks in a row 🔥
```

**Why this works:**
1. **"4 new findings"** — curiosity trigger. What's new?
2. **Quick win section** — 4.9 GB in 3 seconds. Immediate dopamine.
3. **"Needs your attention"** — the real value. Things you forgot about.
4. **Streak counter** — gamification. Don't break the chain.
5. **All-time total** — progress feels meaningful.

#### `zacxiom monthly` — The Deep Clean

```
$ zacxiom monthly

╔══════════════════════════════════════════════════════════╗
║           MONTHLY DEEP STORAGE AUDIT                    ║
║           June 2026                                      ║
╚══════════════════════════════════════════════════════════╝

  📊 MONTHLY SUMMARY
  ─────────────────────────────────────────────────────────
  Storage freed this month:     31.2 GB
  New storage accumulated:      18.7 GB
  Net change:                  -12.5 GB ✅

  🏆 TOP SAVINGS THIS MONTH
  ─────────────────────────────────────────────────────────
  Week 1:  Docker cleanup               12.4 GB
  Week 2:  Old project archive           8.2 GB
  Week 3:  Cargo + npm cache             5.1 GB
  Week 4:  Browser cache + downloads     5.5 GB
  ─────────────────────────────────
  Total:                                 31.2 GB

  ⚠️  DORMANT ASSETS (untouched 90+ days)
  ─────────────────────────────────────────────────────────
  ubuntu-dev.qcow2             4.6 GB  (last used: Jul 2025)
  research-2024.tar.gz        12.1 GB  (last used: Dec 2024)
  defi-dataset/                8.3 GB  (last used: Jan 2025)
  ─────────────────────────────────
  25.0 GB in dormant assets — consider archiving

  📈 FASTEST GROWING (this month)
  ─────────────────────────────────────────────────────────
  Docker layers               +12.4 GB  │██████████████████████│
  Cargo registry               +2.3 GB  │████                  │
  AI model checkpoints         +8.1 GB  │██████████████        │

  🎯 RECOMMENDATION
  ─────────────────────────────────────────────────────────
  Archive dormant assets to external drive → free 25 GB
  Run: zacxiom clean safe                  → free ~5 GB cache
```

#### `zacxiom trend` — The Intelligence Layer

```
$ zacxiom trend

╔══════════════════════════════════════════════════════════╗
║           STORAGE TREND — LAST 12 WEEKS                 ║
╚══════════════════════════════════════════════════════════╝

  TOTAL STORAGE GROWTH
  ─────────────────────────────────────────────────────────
  Week  1:  ██████          +3.2 GB
  Week  2:  ████████████    +6.1 GB
  Week  3:  ████            +2.1 GB
  Week  4:  ██████████████  +12.4 GB  ← Docker build spike
  Week  5:  ██████          +3.5 GB
  ...

  BY DOMAIN (cumulative growth)
  ─────────────────────────────────────────────────────────
  Docker      ████████████████████████████  +28.4 GB  ← growing fastest
  AI Models   ██████████████████            +18.2 GB  ← training checkpoints
  Cargo       ██████                         +6.3 GB
  Downloads   ████████████████████████████  +31.2 GB  ← largest absolute
  Browser     ████                           +4.1 GB

  🔮 PROJECTION (if trends continue)
  ─────────────────────────────────────────────────────────
  In 30 days:   Docker +10 GB, AI Models +6 GB
  In 90 days:   ~48 GB additional storage needed
  Current free: 42 GB → ⚠️ May run low in ~80 days

  💡 INSIGHT
  ─────────────────────────────────────────────────────────
  Downloads is your #1 storage consumer but you rarely clean it.
  Docker is growing 3× faster than everything else.
  Consider: `docker system prune` or Zacxiom Docker domain clean.
```

**This is the killer feature.** It answers: "Why is my SSD filling up?" not with a snapshot, but with a STORY. Users see what's growing, how fast, and what to do about it.

---

## 2. STORAGE INTELLIGENCE

### The Three Layers of Understanding

```
Layer 1: WHAT is using space?         ← du, ncdu do this
Layer 2: WHAT is SAFE to clean?       ← Zacxiom v5.x does this
Layer 3: WHAT is GROWING and WHY?     ← Zacxiom v7.0 should do this
```

Layer 3 is what no tool does today.

### Growth Tracking Architecture

```rust
struct StorageSnapshot {
    timestamp: DateTime,
    domains: HashMap<DomainId, DomainSnapshot>,
}

struct DomainSnapshot {
    total_size: u64,
    file_count: u64,
    new_since_last: u64,
    growth_rate_bytes_per_day: f64,
    largest_items: Vec<Item>,
}
```

Weekly snapshots stored in `~/.local/share/zacxiom/history/` — tiny JSON, negligible storage.

### The "Why Is My SSD Full?" Engine

```
Input: 12 weeks of snapshots
Output: Root cause analysis

Algorithm:
1. Find domain with highest absolute growth → "Docker grew 28 GB"
2. Find domain with highest growth RATE → "AI models growing 3× faster"
3. Find new domains that didn't exist before → "You started using Ollama"
4. Find dormant assets unchanged for 90+ days → "Old VM you forgot about"
5. Correlate with user activity → "Docker spike matches that hackathon week"
```

---

## 3. DEVELOPER WORKFLOWS

### Rust Developer (You)

```
Weekly pattern:
  cargo build → target/ grows
  cargo update → registry grows
  New project → even more target/

Zacxiom should detect:
  ✓ Cargo registry cache → SAFE (★★★★★)
  ✓ target/ directories → SAFE (★★★★★)
  ✓ .rustup old toolchains → SAFE (★★★★★)
  ✓ Stale projects with no commits → REVIEW (★★★☆☆)
  
Value: 5-20 GB reclaimable every 1-2 weeks
```

### Docker User (You)

```
Weekly pattern:
  docker build → layers accumulate
  docker pull → images accumulate
  docker run → volumes accumulate
  
Zacxiom should detect:
  ✓ Orphaned images (no container uses them) → SAFE (★★★★☆)
  ✓ Build cache → SAFE (★★★★★)
  ✓ Dangling volumes → REVIEW (★★★☆☆)
  ✓ Old images (pulled >90d ago, never used) → SAFE (★★★★★)
  
Value: 20-100 GB reclaimable monthly
```

### AI/ML Builder (You)

```
Weekly pattern:
  Training runs → checkpoint files (*.pt, *.safetensors)
  Dataset downloads → large archives
  Model downloads → HuggingFace cache (~/.cache/huggingface/)
  Experiment outputs → logs, metrics, visualizations
  
Zacxiom should detect:
  ✓ HuggingFace cache → SAFE (★★★★★)
  ✓ Old checkpoints (only keep best N) → REVIEW (★★★☆☆)
  ✓ Stale datasets (>90d untouched) → REVIEW (★★☆☆☆)
  ✓ Ollama models (unused versions) → REVIEW (★★★☆☆)
  
Value: 10-200 GB reclaimable (AI is the heaviest storage consumer)
```

### Trading/Research (You)

```
Weekly pattern:
  Market data downloads → large .csv/.parquet files
  Backtest outputs → logs, reports
  Historical data → rarely accessed after initial analysis
  
Zacxiom should detect:
  ✓ Old market data (>90d, re-downloadable) → SAFE (★★★★☆)
  ✓ Backtest artifacts (>30d) → SAFE (★★★★★)
  ✓ Duplicate datasets across projects → REVIEW (★★☆☆☆)
  
Value: 5-50 GB reclaimable
```

### Browser-Heavy Research (You)

```
Weekly pattern:
  Many tabs → large browser cache
  PDF downloads → accumulate in ~/Downloads
  Research papers → read once, never deleted
  
Zacxiom should detect:
  ✓ Browser cache → SAFE (★★★★★)
  ✓ Old PDFs in Downloads (>90d) → REVIEW (★★★☆☆)
  ✓ Old screenshots → REVIEW (★★★☆☆)
```

---

## 4. DOMAIN ARCHITECTURE

### The Five-Domain Model

Users don't think in filesystem paths. They think in domains.

| Domain | Sub-domains | Safety | Typical Size | Growth Rate |
|---|---|---|---|---|
| **Developer** | Cargo, npm, pip, Docker, Gradle, Go | ★★★★★ | 20-150 GB | Fast |
| **Gaming** | Steam, Proton, DXVK, Lutris, Mesa | ★★★★☆ | 10-80 GB | Medium |
| **AI** | HuggingFace, Ollama, Checkpoints, Datasets | ★★★☆☆ | 10-200 GB | Very Fast |
| **System** | Pacman, Flatpak, Snap, Logs, Tmp | ★★★★★ | 5-30 GB | Slow |
| **Downloads** | ISOs, Archives, PDFs, Installers | ★★☆☆☆ | 5-50 GB | Variable |

### Domain Intelligence

Each domain has unique characteristics:

```
Developer Domain
  Safety: Very High — everything regenerable
  Growth: Fast — every build adds artifacts
  User mindset: "I know I can clean this, I just forget"
  Strategy: Auto-suggest weekly

Gaming Domain
  Safety: High — shaders regenerate, saves are separate
  Growth: Medium — new games add shaders, Proton prefixes
  User mindset: "I don't want to re-download 80 GB games"
  Strategy: Distinguish cache from game data

AI Domain
  Safety: Medium — checkpoints may be valuable
  Growth: Very Fast — training generates GB per run
  User mindset: "Are these checkpoints still useful?"
  Strategy: Show checkpoint age + best metrics

Downloads Domain
  Safety: Low — user needs to review
  Growth: Variable — burst when downloading ISOs/datasets
  User mindset: "I forgot these were here"
  Strategy: Age-based flagging, never auto-clean

System Domain
  Safety: High — package managers regenerate
  Growth: Slow — accumulates over months
  User mindset: "I don't think about this"
  Strategy: Monthly reminder
```

### The "Never Uninstall" Test

A tool passes this test when, after 3 months, the user can answer:

1. **What did Zacxiom find this week that I didn't know about?** → Discovery value
2. **How much storage did Zacxiom help me reclaim this month?** → Measurable impact
3. **Do I trust Zacxiom enough to run `clean` without auditing?** → Trust
4. **Would I notice if Zacxiom was gone?** → Dependency

---

## 5. TRUST

### The Trust Ladder

```
Week 1:  Run `zacxiom scan` out of curiosity
         → "Hmm, 24 GB reclaimable? Interesting."

Week 2:  Run `zacxiom weekly` and see new findings
         → "Oh, I forgot about that old VM. Nice catch."

Week 3:  Run `zacxiom clean safe` for the first time
         → "That was easy. Nothing broke."

Week 4:  Run `zacxiom clean safe` without thinking
         → Habit formed.

Week 8:  Start using `--review` for deeper cleaning
         → Trust established.

Week 12: Zacxiom is just part of the Monday routine
         → Never uninstall.
```

### Trust Accelerators

1. **Preview everything** — Never delete without showing exactly what
2. **Stars, not jargon** — ★★★★★ is universal. "Fully regenerable cache" is engineering.
3. **Explain consequences** — Not "this is safe" but "next cargo build takes 3 more minutes"
4. **Undo everything** — Every clean creates a snapshot. Peace of mind.
5. **Conservative by default** — `clean safe` only touches ★★★★★ items. Everything else requires explicit flags.
6. **Never touch active files** — Process detection prevents accidents.
7. **Never touch protected paths** — /etc, /boot, SSH keys are untouchable.

---

## 6. KILLER FEATURES

### The 5 Features That Create Dependency

#### #1: `zacxiom weekly` — The Habit Command (Impact: 10/10)

A single command that delivers new value every week. Shows what's new, what's safe, and what needs attention. Designed to be checked Monday morning like email.

**Why it creates dependency:** It's the first thing you check for storage health. When it's gone, you feel blind.

#### #2: Storage Timeline — Growth Tracking (Impact: 9/10)

Answers "Why is my SSD filling up?" by showing growth over time, not just current state. Identifies which domains are growing fastest and projects future storage needs.

**Why it creates dependency:** No other tool does this. Once you've seen your storage story over 12 weeks, a single `du -sh` feels primitive.

#### #3: Forgotten Storage Detection (Impact: 9/10)

Finds abandoned projects, dormant VMs, old downloads, and stale datasets — things the user literally forgot existed. These are often 2-5× larger than all caches combined.

**Why it creates dependency:** Discovery creates delight. "I forgot I had that 12 GB VM!" is a moment of genuine value.

#### #4: Domain-Based Intelligence (Impact: 8/10)

Organizes storage into domains users actually understand (Developer, Gaming, AI, System, Downloads) instead of filesystem paths. Each domain has unique safety characteristics and recommendations.

**Why it creates dependency:** Makes storage understandable. Users see "Docker: 28 GB growth this month" instead of 10,000 individual files.

#### #5: The 5-Question Trust Card (Impact: 8/10)

Every cleanup recommendation answers: What is it? Why is it safe? What happens if deleted? How much space? Can I undo? With ★★★★★ safety tiers.

**Why it creates dependency:** Trust is earned one explanation at a time. After 10 weeks of accurate predictions, users stop doubting and start depending.

---

## 7. ROADMAP

```
v6.1.0 — SEE WHAT MATTERS ──────────────────────────────────
  Theme: "Default scan actually finds the big stuff"
  
  ✓ Expand default scan roots (cargo, npm, docker, steam, rustup)
  ✓ 10 new domain patterns (Steam, Proton, Docker, Flatpak, AI, etc.)  
  ✓ Five-domain architecture (Developer, Gaming, AI, System, Downloads)
  ✓ ★★★★★ Safety tier system (trust foundation)
  ✓ Human-readable domain summaries
  
  Effect: Default scan finds 5-10× more storage
  Time:   2-3 days of focused work

v6.2.0 — BUILD TRUST ───────────────────────────────────────
  Theme: "I trust this tool enough to run clean without auditing"
  
  ✓ --dry-run preview mode (show exactly what would happen)
  ✓ 5-Question explainability cards
  ✓ --domains developer,docker filtering
  ✓ Age-based abandonment detection (>180d stale projects)
  ✓ Dormant large file detection (>1GB, >90d untouched)
  
  Effect: Users confidently run `clean safe` weekly
  Time:   1-2 weeks

v7.0.0 — HABIT FORMATION ───────────────────────────────────
  Theme: "The Linux maintenance companion developers use weekly"
  
  ✓ zacxiom weekly command (habit-forming report)
  ✓ Storage timeline / trend tracking (12-week history)
  ✓ Growth projection ("40 GB free → may run low in 80 days")
  ✓ Duplicate file detection (same name + size across projects)
  ✓ Version accumulation detection (old AppImages, toolchains)
  ✓ systemd timer for auto weekly scan
  ✓ Streak counter + all-time stats ("247 GB across 31 cleans")
  
  Effect: Zacxiom becomes a weekly habit, like checking email
  Time:   2-4 weeks

v8.0.0+ — INTELLIGENCE PLATFORM ─────────────────────────────
  Theme: "The single source of truth for Linux storage health"
  
  → Interactive TUI domain browser
  → Storage anomaly detection (unusual growth spikes)
  → Per-project storage profiles
  → Multi-machine storage comparison
  → Export/API for monitoring dashboards
```

---

## THE POSITIONING

Don't say: "Zacxiom is a cache cleaner for Linux."

Say:

> **Zacxiom is how Linux developers understand and manage their storage.**
>
> It finds what you forgot. It explains why it's safe. It tracks what's growing.
> You run `zacxiom weekly` on Monday morning like you check your email.
>
> Not a cleaner. A companion.

The difference isn't marketing. It's what you build.

A cleaner adds pattern matching. A companion adds discovery, explanation, history, and trust.

---

## THE FINAL QUESTION

After 6 months of using Zacxiom weekly:

**Does the user feel like they understand their storage better than before?**

If yes → Never uninstall.  
If no → It's just another cleaner.

Everything in this strategy serves that one outcome.
