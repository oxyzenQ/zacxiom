# Backup Integration Guide

Zacxiom integrates with backup tools (restic, borg, rsync) to ensure
your cleanup operations don't conflict with your backup strategy.

## Philosophy

**Backup before clean, not after.** If you clean first, the deleted files
are in zacxiom's trash (recoverable for 30 days). If you backup first,
your backup contains the files that were about to be cleaned — wasting space.

The recommended pattern: **backup → clean → verify backup**.

---

## restic Integration

### Pre-clean backup hook

Run this before `zacxiom clean` to ensure a backup exists:

```bash
#!/bin/bash
# pre-clean-backup.sh — backup before zacxiom clean

REPO="/backup/restic"
SOURCE="/home/user"

# Create snapshot before cleaning
restic -r "$REPO" backup "$SOURCE" \
    --exclude-caches \
    --exclude "/home/user/.local/share/zacxiom" \
    --tag "pre-zacxiom-clean"

# Now run zacxiom
zacxiom clean --smart --yes

# Verify backup is intact
restic -r "$REPO" snapshots --latest 1
```

### Cron example (weekly backup + clean)

```bash
# /etc/cron.weekly/zacxiom-backup-clean
#!/bin/bash
set -euo pipefail

REPO="/backup/restic"
SOURCE="/home/user"

# 1. Backup
restic -r "$REPO" backup "$SOURCE" \
    --exclude-caches \
    --exclude "/home/user/.local/share/zacxiom"

# 2. Clean
zacxiom clean --smart --yes --quiet

# 3. Prune old backups (keep 4 weekly)
restic -r "$REPO" forget --keep-weekly 4 --prune

# 4. Audit log
echo "$(date): backup + clean complete" >> /var/log/zacxiom-cron.log
```

---

## Borg Integration

### Pre-clean backup

```bash
#!/bin/bash
# borg pre-clean backup

REPO="/backup/borg"
SOURCE="/home/user"

# Create archive before cleaning
borg create --stats \
    --exclude-caches \
    --exclude "/home/user/.local/share/zacxiom" \
    "$REPO::pre-clean-{now}" \
    "$SOURCE"

# Run zacxiom
zacxiom clean --smart --yes

# Prune old archives
borg prune --keep-weekly 4 "$REPO"
```

---

## rsync Integration

For simple rsync-based backups:

```bash
#!/bin/bash
# rsync pre-clean backup

DEST="/backup/rsync/$(date +%Y-%m-%d)"
SOURCE="/home/user"

# Mirror to backup (hardlinks for space efficiency)
rsync -aHAX --delete \
    --exclude "/home/user/.local/share/zacxiom" \
    --link-dest="/backup/rsync/current" \
    "$SOURCE/" "$DEST/"

# Update current symlink
ln -sfn "$DEST" "/backup/rsync/current"

# Now clean
zacxiom clean --smart --yes
```

---

## Important: Exclude zacxiom's data from backups

Zacxiom stores:
- **Snapshots + trash**: `~/.local/share/zacxiom/` (XDG_DATA_HOME)
- **Memory**: `~/.local/share/zacxiom/memory.json`
- **Audit log**: `~/.local/share/zacxiom/audit.log`
- **Scan cache**: `~/.cache/zacxiom/` (XDG_CACHE_HOME — disposable)

**Exclude `~/.local/share/zacxiom/` from backups** — it contains trash copies
of files you already deleted. Backing these up wastes space and defeats the
purpose of cleaning.

The scan cache in `~/.cache/zacxiom/` is disposable — no need to back it up.

---

## Verification

After backup + clean, verify:

```bash
# Check audit log shows the clean operation
tail -5 ~/.local/share/zacxiom/audit.log | jq .

# Verify snapshot exists for undo
zacxiom snapshot list

# Test undo (restore last clean)
zacxiom undo --dry-run  # hypothetical — zacxiom doesn't have --dry-run on undo
# Instead: zacxiom undo --list to see available snapshots
```

---

## Recovery Scenario

If you accidentally clean something important:

1. **Within 30 days**: `zacxiom undo` — restores from trash
2. **After 30 days** (snapshot pruned): restore from backup
3. **Audit trail**: check `~/.local/share/zacxiom/audit.log` for what was cleaned

```bash
# Find what was cleaned on a specific date
grep "2026-07-01" ~/.local/share/zacxiom/audit.log | jq .
```

---

## Best Practices

1. **Backup frequency > clean frequency** — backup daily, clean weekly
2. **Test restore** — verify your backup works before relying on it
3. **Monitor audit log** — check for unexpected clean operations
4. **Keep zacxiom trash** — don't manually delete `~/.local/share/zacxiom/trash/`
5. **Use --quiet for cron** — reduces log noise
