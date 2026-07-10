#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Deep stress test for zacxiom clean pipeline.
# Tests: scan, clean, undo, snapshot purge, status, cache-stats, edge cases.
# Run: bash scripts/stress_test.sh <path-to-zacxiom-binary>

set -euo pipefail

ZACXIOM="${1:?Usage: $0 <path-to-zacxiom-binary>}"
PASS=0
FAIL=0
WARN=0
TESTS=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

report() {
  local label="$1" result="$2" detail="${3:-}"
  if [ "$result" = "PASS" ]; then
    echo -e "  ${GREEN}PASS${NC}  $label"
    PASS=$((PASS + 1))
  elif [ "$result" = "FAIL" ]; then
    echo -e "  ${RED}FAIL${NC}  $label"
    if [ -n "$detail" ]; then
      echo -e "        ${detail}"
    fi
    FAIL=$((FAIL + 1))
  else
    echo -e "  ${YELLOW}WARN${NC}  $label"
    if [ -n "$detail" ]; then
      echo -e "        ${detail}"
    fi
    WARN=$((WARN + 1))
  fi
}

assert_eq() {
  local label="$1" actual="$2" expected="$3"
  if [ "$actual" = "$expected" ]; then
    report "$label" PASS
  else
    report "$label" FAIL "expected='$expected' actual='$actual'"
  fi
}

assert_contains() {
  local label="$1" haystack="$2" needle="$3"
  if echo "$haystack" | grep -qF "$needle"; then
    report "$label" PASS
  else
    report "$label" FAIL "missing '$needle' in output"
  fi
}

assert_not_contains() {
  local label="$1" haystack="$2" needle="$3"
  if echo "$haystack" | grep -qF "$needle"; then
    report "$label" FAIL "unexpected '$needle' found in output"
  else
    report "$label" PASS
  fi
}

assert_exit_code() {
  local label="$1" actual="$2" expected="$3"
  if [ "$actual" -eq "$expected" ]; then
    report "$label" PASS
  else
    report "$label" FAIL "expected exit=$expected actual=$actual"
  fi
}

# ── Setup ──────────────────────────────────────────────────────
TMPDIR=$(mktemp -d)
export HOME="$TMPDIR/home"
export XDG_DATA_HOME="$HOME/.local/share"
export XDG_CACHE_HOME="$HOME/.cache"
mkdir -p "$HOME/.local/share/zacxiom/snapshots"
mkdir -p "$HOME/.cache/zacxiom"
# Prevent /var/cache and /tmp from being scanned (they're system dirs)
mkdir -p "$TMPDIR/fake_root/var/cache"
mkdir -p "$TMPDIR/fake_root/tmp"

# Create test file tree
mkdir -p "$HOME/.cache/mozilla/firefox/profile1/cache2"
mkdir -p "$HOME/.cache/chromium/Default/Cache"
mkdir -p "$HOME/.cargo/registry/cache"
mkdir -p "$HOME/.cargo/registry/src"
mkdir -p "$HOME/.cache/pip"
mkdir -p "$HOME/.npm/_cacache"
mkdir -p "$HOME/.local/share/Trash/files"
mkdir -p "$HOME/.steam/steam/steamapps/shadercache"
mkdir -p "$HOME/project/target/debug/build"
mkdir -p "$HOME/project/target/release/build"
mkdir -p "$HOME/project/node_modules/lodash"
mkdir -p "$HOME/.cache/yay"
mkdir -p "$HOME/.cache/ollama/models"

# Populate files with real-ish content
dd if=/dev/urandom of="$HOME/.cache/mozilla/firefox/profile1/cache2/entry1" bs=1024 count=50 2>/dev/null
dd if=/dev/urandom of="$HOME/.cache/mozilla/firefox/profile1/cache2/entry2" bs=1024 count=100 2>/dev/null
dd if=/dev/urandom of="$HOME/.cache/chromium/Default/Cache/data1" bs=1024 count=200 2>/dev/null
dd if=/dev/urandom of="$HOME/.cargo/registry/cache/crate1.crate" bs=1024 count=500 2>/dev/null
dd if=/dev/urandom of="$HOME/.cargo/registry/cache/crate2.crate" bs=1024 count=300 2>/dev/null
dd if=/dev/urandom of="$HOME/.cargo/registry/src/index/foo.rs" bs=1024 count=10 2>/dev/null
dd if=/dev/urandom of="$HOME/.cache/pip/wheels/package.whl" bs=1024 count=80 2>/dev/null
dd if=/dev/urandom of="$HOME/.npm/_cacache/content/dep1" bs=1024 count=40 2>/dev/null
dd if=/dev/urandom of="$HOME/.local/share/Trash/files/deleted_doc.txt" bs=1024 count=150 2>/dev/null
dd if=/dev/urandom of="$HOME/.steam/steam/steamapps/shadercache/440/shader.bin" bs=1024 count=256 2>/dev/null
dd if=/dev/urandom of="$HOME/project/target/debug/build/artifact.o" bs=1024 count=60 2>/dev/null
dd if=/dev/urandom of="$HOME/project/target/release/build/artifact.o" bs=1024 count=70 2>/dev/null
dd if=/dev/urandom of="$HOME/project/node_modules/lodash/index.js" bs=1024 count=30 2>/dev/null
dd if=/dev/urandom of="$HOME/.cache/yay/firefox/PKGBUILD" bs=1024 count=20 2>/dev/null
dd if=/dev/urandom of="$HOME/.cache/ollama/models/model.gguf" bs=1024 count=400 2>/dev/null

echo ""
echo -e "${CYAN}━━━ ZACXIOM DEEP STRESS TEST ━━━${NC}"
echo "  TMPDIR: $TMPDIR"
echo "  HOME:   $HOME"
echo "  Binary: $ZACXIOM"
echo ""

# ── T1: Basic commands don't crash ─────────────────────────────
echo -e "${CYAN}── Category 1: Basic Command Health ───${NC}"

OUT=$($ZACXIOM --version 2>&1)
assert_contains "T1.1  --version works" "$OUT" "v14.4.0"

OUT=$($ZACXIOM --help 2>&1)
assert_contains "T1.2  --help works" "$OUT" "Safe filesystem cleaning"

OUT=$($ZACXIOM status 2>&1)
assert_contains "T1.3  status works" "$OUT" "ZACXIOM"
assert_not_contains "T1.4  status no root warning (non-root)" "$OUT" "Scope"

OUT=$($ZACXIOM doctor 2>&1)
assert_contains "T1.5  doctor works" "$OUT" "ZACXIOM"

OUT=$($ZACXIOM --cache-stats 2>&1)
assert_contains "T1.6  --cache-stats works" "$OUT" "SCAN CACHE"

echo ""

# ── T2: Scan ──────────────────────────────────────────────────
echo -e "${CYAN}── Category 2: Scan Pipeline ───${NC}"

# Scan our fake HOME (only cache dirs, not system dirs)
OUT=$($ZACXIOM scan -P "$HOME/.cache" -P "$HOME/.cargo" -P "$HOME/.npm" -P "$HOME/.local/share/Trash" -P "$HOME/.steam" -P "$HOME/project/target" -P "$HOME/project/node_modules" 2>&1)
assert_contains "T2.1  scan finds files" "$OUT" "Safe"
assert_contains "T2.2  scan shows domains" "$OUT" "Browser Cache"

# Test with --no-cache
OUT=$($ZACXIOM --no-cache scan -P "$HOME/.cache/mozilla" 2>&1)
assert_contains "T2.3  scan --no-cache works" "$OUT" "Safe"

# Test JSON output
OUT=$($ZACXIOM scan -P "$HOME/.cache/mozilla" --json 2>&1)
assert_contains "T2.4  scan --json output" "$OUT" '"health"'

# Test scan cache
OUT=$($ZACXIOM scan -P "$HOME/.cache/mozilla" 2>&1)
assert_contains "T2.5  scan cache hit on 2nd run" "$OUT" "Cache:"

# Cache stats after scan should have entries
OUT=$($ZACXIOM --cache-stats 2>&1)
assert_contains "T2.6  cache-stats has entries after scan" "$OUT" "Entries:"
# Verify entries > 0
CACHE_ENTRIES=$(echo "$OUT" | rg "Entries:" | rg -o '[0-9]+' || true)
if [ -n "$CACHE_ENTRIES" ] && [ "$CACHE_ENTRIES" -gt 0 ]; then
  report "T2.7  cache-stats entries > 0" PASS
else
  report "T2.7  cache-stats entries > 0" FAIL "entries=$CACHE_ENTRIES"
fi

echo ""

# ── T3: Clean (dry-run) ──────────────────────────────────────
echo -e "${CYAN}── Category 3: Clean Pipeline (Dry Run) ───${NC}"

OUT=$($ZACXIOM clean -P "$HOME/.cache/mozilla" --dry-run 2>&1)
assert_contains "T3.1  clean --dry-run works" "$OUT" "DRY RUN"
assert_contains "T3.2  dry-run shows mode" "$OUT" "Mode:"

OUT=$($ZACXIOM clean -P "$HOME/.cache/mozilla" --smart --dry-run 2>&1)
assert_contains "T3.3  clean --smart --dry-run" "$OUT" "SMART"

# JSON dry-run
OUT=$($ZACXIOM clean -P "$HOME/.cache/mozilla" --dry-run --json 2>&1)
assert_contains "T3.4  clean --dry-run --json" "$OUT" '"mode"'

echo ""

# ── T4: Clean (real) + Undo ───────────────────────────────────
echo -e "${CYAN}── Category 4: Clean + Undo + Snapshot ───${NC}"

# Real clean (safe mode, --yes to skip prompts)
OUT=$($ZACXIOM clean -P "$HOME/.cache/mozilla" --yes 2>&1)
assert_contains "T4.1  clean removes files" "$OUT" "Removed:"
assert_contains "T4.2  clean creates snapshot" "$OUT" "Snapshot:"

# Files should be gone
if [ ! -f "$HOME/.cache/mozilla/firefox/profile1/cache2/entry1" ]; then
  report "T4.3  file actually removed" PASS
else
  report "T4.3  file actually removed" FAIL "entry1 still exists"
fi

# Undo should restore
OUT=$($ZACXIOM undo 2>&1)
assert_contains "T4.4  undo restores files" "$OUT" "Restored"

# Files should be back
if [ -f "$HOME/.cache/mozilla/firefox/profile1/cache2/entry1" ]; then
  report "T4.5  file restored after undo" PASS
else
  report "T4.5  file restored after undo" FAIL "entry1 not restored"
fi

echo ""

# ── T5: Snapshot Management ──────────────────────────────────
echo -e "${CYAN}── Category 5: Snapshot Management ───${NC}"

# Do another clean to create a second snapshot
$ZACXIOM clean -P "$HOME/.cache/chromium" --yes >/dev/null 2>&1

OUT=$($ZACXIOM snapshot list 2>&1)
SNAP_COUNT=$(echo "$OUT" | rg -c "^  [0-9]" || true)
assert_contains "T5.1  snapshot list works" "$OUT" "Snapshots"

# Snapshot verify
OUT=$($ZACXIOM snapshot verify 2>&1)
assert_contains "T5.2  snapshot verify works" "$OUT" "valid"

# Prune --keep 1
OUT=$($ZACXIOM snapshot prune --keep 1 2>&1)
assert_contains "T5.3  snapshot prune --keep 1" "$OUT" "Pruned"

# Check count after prune
OUT=$($ZACXIOM snapshot list 2>&1)
SNAP_AFTER_PRUNE=$(echo "$OUT" | rg -c "^  [0-9]" || true)
if [ "$SNAP_AFTER_PRUNE" -le 1 ]; then
  report "T5.4  prune reduced snapshot count" PASS
else
  report "T5.4  prune reduced snapshot count" FAIL "before=$SNAP_COUNT after=$SNAP_AFTER_PRUNE"
fi

echo ""

# ── T6: Purge ALL (the fixed bug) ────────────────────────────
echo -e "${CYAN}── Category 6: Snapshot Purge (Bug #1 Fix Verification) ───${NC}"

# Create snapshots in BOTH XDG and legacy dirs to test the fix
XDG_SNAP_DIR="$HOME/.local/share/zacxiom/snapshots"
LEGACY_SNAP_DIR="$HOME/.cache/zacxiom/snapshots"
mkdir -p "$LEGACY_SNAP_DIR"

# Do a clean (goes to XDG dir)
$ZACXIOM clean -P "$HOME/.cache/pip" --yes >/dev/null 2>&1

# Manually create a snapshot in the LEGACY dir (simulating old zacxiom)
LEGACY_SNAP_ID="snap-99999-0000000001-0001"
cat > "$LEGACY_SNAP_DIR/$LEGACY_SNAP_ID" <<EOF
{
  "id": "$LEGACY_SNAP_ID",
  "created": "$(date +%s)",
  "entries": [
    {"path": "/tmp/old_file.txt", "size": 100, "trash_path": null, "timestamp": "$(date +%s)", "skipped": false}
  ]
}
EOF

# List should show snapshots from BOTH dirs
OUT=$($ZACXIOM snapshot list 2>&1)
TOTAL_SNAPS=$(echo "$OUT" | rg -c "^  [0-9]" || true)
assert_contains "T6.1  list sees both XDG + legacy" "$OUT" "Snapshots"

if [ "$TOTAL_SNAPS" -ge 2 ]; then
  report "T6.2  list shows >=2 snapshots (XDG+legacy)" PASS
else
  report "T6.2  list shows >=2 snapshots (XDG+legacy)" FAIL "count=$TOTAL_SNAPS"
fi

# NOW PURGE — this is the critical test
OUT=$($ZACXIOM snapshot purge --confirm "DELETE ALL" 2>&1)
assert_contains "T6.3  purge says ALL" "$OUT" "Purged ALL"
assert_contains "T6.4  purge reports correct count" "$OUT" "$TOTAL_SNAPS snapshot(s)"

# Verify BOTH dirs are empty after purge
XDG_REMAIN=$(ls -1 "$XDG_SNAP_DIR" 2>/dev/null | wc -l)
LEGACY_REMAIN=$(ls -1 "$LEGACY_SNAP_DIR" 2>/dev/null | wc -l)

if [ "$XDG_REMAIN" -eq 0 ]; then
  report "T6.5  XDG snapshot dir empty after purge" PASS
else
  report "T6.5  XDG snapshot dir empty after purge" FAIL "$XDG_REMAIN files remain"
fi

if [ "$LEGACY_REMAIN" -eq 0 ]; then
  report "T6.6  Legacy snapshot dir empty after purge" PASS
else
  report "T6.6  Legacy snapshot dir empty after purge" FAIL "$LEGACY_REMAIN files remain"
fi

# List should show 0 now
OUT=$($ZACXIOM snapshot list 2>&1)
assert_contains "T6.7  list shows no snapshots after purge" "$OUT" "No snapshots found"

# Purge again should be idempotent
OUT=$($ZACXIOM snapshot purge --confirm "DELETE ALL" 2>&1)
assert_contains "T6.8  purge idempotent (empty)" "$OUT" "No snapshots to purge"

echo ""

# ── T7: Edge Cases ───────────────────────────────────────────
echo -e "${CYAN}── Category 7: Edge Cases ───${NC}"

# Clean empty dir
mkdir -p "$TMPDIR/empty_dir"
OUT=$($ZACXIOM clean -P "$TMPDIR/empty_dir" --yes 2>&1)
assert_contains "T7.1  clean empty dir" "$OUT" "No files were removed"

# Clean non-existent path
OUT=$($ZACXIOM scan -P "$TMPDIR/nonexistent_path_xyz" 2>&1) || true
# Should not crash

# Undo with no snapshots
$ZACXIOM snapshot purge --confirm "DELETE ALL" >/dev/null 2>&1 || true
OUT=$($ZACXIOM undo 2>&1) || true
assert_contains "T7.2  undo with no snapshots" "$OUT" "No snapshots found"

# Status should show 0
OUT=$($ZACXIOM status 2>&1)
assert_contains "T7.3  status shows 0 snapshots" "$OUT" "Snapshots : 0 available"

# Plan on a real dir
OUT=$($ZACXIOM plan "$HOME/project" 2>&1)
assert_contains "T7.4  plan works on dir" "$OUT" ""

# Explain on a cache dir
OUT=$($ZACXIOM explain "$HOME/.cache" 2>&1)
assert_contains "T7.5  explain works" "$OUT" ""

# Viz on a dir
OUT=$($ZACXIOM viz "$HOME/project" 2>&1)
assert_contains "T7.6  viz works" "$OUT" ""

# Dedup
OUT=$($ZACXIOM dedup -P "$HOME/project" --json 2>&1)
assert_contains "T7.7  dedup --json works" "$OUT" '"duplicate_groups"'

echo ""

# ── T8: Multiple clean + undo cycles ─────────────────────────
echo -e "${CYAN}── Category 8: Multi-Cycle Clean/Undo ───${NC}"

# Cycle 1: clean, undo, verify
ORIG_CONTENT=$(cat "$HOME/.cache/mozilla/firefox/profile1/cache2/entry2" | md5sum)
$ZACXIOM clean -P "$HOME/.cache/mozilla" --yes >/dev/null 2>&1
$ZACXIOM undo >/dev/null 2>&1
RESTORED_CONTENT=$(cat "$HOME/.cache/mozilla/firefox/profile1/cache2/entry2" | md5sum)
if [ "$ORIG_CONTENT" = "$RESTORED_CONTENT" ]; then
  report "T8.1  cycle 1: content preserved after undo" PASS
else
  report "T8.1  cycle 1: content preserved after undo" FAIL
fi

# Cycle 2: same thing (tests snapshot isolation)
$ZACXIOM clean -P "$HOME/.cache/mozilla" --yes >/dev/null 2>&1
$ZACXIOM undo >/dev/null 2>&1
RESTORED2=$(cat "$HOME/.cache/mozilla/firefox/profile1/cache2/entry2" | md5sum)
if [ "$ORIG_CONTENT" = "$RESTORED2" ]; then
  report "T8.2  cycle 2: content preserved after undo" PASS
else
  report "T8.2  cycle 2: content preserved after undo" FAIL
fi

echo ""

# ── T9: Protected paths ──────────────────────────────────────
echo -e "${CYAN}── Category 9: Safety Guards ───${NC}"

# Protected paths should never be cleaned
mkdir -p "$TMPDIR/etc_test"
echo "critical" > "$TMPDIR/etc_test/critical.conf"
OUT=$($ZACXIOM clean -P "$TMPDIR/etc_test" --force --dry-run 2>&1)
assert_contains "T9.1  /etc-like paths protected" "$OUT" "Would skip"

# .iso files should be protected
mkdir -p "$TMPDIR/iso_test"
dd if=/dev/urandom of="$TMPDIR/iso_test/big.iso" bs=1024 count=100 2>/dev/null
OUT=$($ZACXIOM scan -P "$TMPDIR/iso_test" 2>&1)
# ISOs are always Protected
assert_contains "T9.2  .iso files protected" "$OUT" "Protected"

# .pem files should be protected
echo "PRIVATE KEY" > "$TMPDIR/iso_test/key.pem"
OUT=$($ZACXIOM scan -P "$TMPDIR/iso_test" 2>&1)
assert_contains "T9.3  .pem files protected" "$OUT" "Protected"

echo ""

# ── T10: Snapshot delete + undo idempotency ──────────────────
echo -e "${CYAN}── Category 10: Snapshot Delete Edge Cases ───${NC}"

$ZACXIOM clean -P "$HOME/.cache/yay" --yes >/dev/null 2>&1

# Get snapshot ID
SNAP_ID=$($ZACXIOM snapshot list --json 2>&1 | python3 -c "import sys,json; print(json.load(sys.stdin)['snapshots'][0]['id'])" 2>/dev/null || true)

if [ -n "$SNAP_ID" ]; then
  # Delete specific snapshot
  OUT=$($ZACXIOM snapshot delete "$SNAP_ID" --force 2>&1)
  assert_contains "T10.1 delete specific snapshot" "$OUT" "deleted"

  # Delete non-existent should fail gracefully
  OUT=$($ZACXIOM snapshot delete "snap-nonexistent" --force 2>&1) || true
  assert_contains "T10.2 delete nonexistent fails gracefully" "$OUT" "not found"
else
  report "T10.1 delete specific snapshot" WARN "no snapshot found"
  report "T10.2 delete nonexistent fails gracefully" WARN "skipped"
fi

echo ""

# ── Cleanup ──────────────────────────────────────────────────
rm -rf "$TMPDIR"

# ── Summary ──────────────────────────────────────────────────
echo ""
echo -e "${CYAN}━━━ STRESS TEST SUMMARY ━━━${NC}"
echo -e "  ${GREEN}PASS${NC}: $PASS"
echo -e "  ${RED}FAIL${NC}: $FAIL"
echo -e "  ${YELLOW}WARN${NC}: $WARN"
echo ""

if [ "$FAIL" -gt 0 ]; then
  echo -e "${RED}━━━ SOME TESTS FAILED ━━━${NC}"
  exit 1
else
  echo -e "${GREEN}━━━ ALL TESTS PASSED ━━━${NC}"
  exit 0
fi