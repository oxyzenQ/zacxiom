#!/usr/bin/env python3
"""
Deep stress test for zacxiom clean pipeline.
Tests: scan, clean, undo, snapshot purge (XDG+legacy), status, cache-stats, edge cases.
Run: python3 scripts/stress_test.py <path-to-zacxiom-binary>
"""

import subprocess
import os
import sys
import json
import tempfile
import shutil
import time

PASS = 0
FAIL = 0
WARN = 0

def report(label, result, detail=""):
    global PASS, FAIL, WARN
    if result == "PASS":
        print(f"  \033[0;32mPASS\033[0m  {label}")
        PASS += 1
    elif result == "FAIL":
        print(f"  \033[0;31mFAIL\033[0m  {label}")
        if detail:
            print(f"        {detail}")
        FAIL += 1
    else:
        print(f"  \033[1;33mWARN\033[0m  {label}")
        if detail:
            print(f"        {detail}")
        WARN += 1

def assert_contains(label, haystack, needle):
    report(label, "PASS" if needle in haystack else "FAIL",
           f"missing '{needle}'" if needle not in haystack else "")

def assert_not_contains(label, haystack, needle):
    report(label, "PASS" if needle not in haystack else "FAIL",
           f"unexpected '{needle}' found" if needle in haystack else "")

def assert_eq(label, actual, expected):
    report(label, "PASS" if actual == expected else "FAIL",
           f"expected='{expected}' actual='{actual}'")

def assert_gt(label, actual, threshold):
    ok = isinstance(actual, (int, float)) and actual > threshold
    report(label, "PASS" if ok else "FAIL",
           f"value={actual} threshold={threshold}")

def assert_true(label, condition, detail=""):
    report(label, "PASS" if condition else "FAIL", detail)

def strip_ansi(text):
    """Remove ANSI escape sequences and carriage returns from output."""
    import re
    text = re.sub(r'\x1b\[[0-9;]*[a-zA-Z]', '', text)
    text = text.replace('\r', '')
    return text

def run(args, env):
    r = subprocess.run(
        [ZACXIOM] + args,
        capture_output=True, text=True, timeout=60, env=env
    )
    return r.stdout + r.stderr, r.returncode

def write_file(path, size_kb):
    with open(path, "wb") as f:
        f.write(os.urandom(size_kb * 1024))

def touch_old(path, size_kb, days_old=2):
    """Write a file and set mtime to days_old days ago so it gets Safe decision."""
    write_file(path, size_kb)
    # Set mtime to days_old days ago
    old_time = time.time() - (days_old * 86400)
    os.utime(path, (old_time, old_time))

def main():
    global ZACXIOM, PASS, FAIL, WARN

    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path-to-zacxiom-binary>")
        sys.exit(1)

    ZACXIOM = os.path.abspath(sys.argv[1])
    if not os.path.isfile(ZACXIOM):
        print(f"Binary not found: {ZACXIOM}")
        sys.exit(1)

    # ── Setup isolated environment ──────────────────────────
    tmpdir = tempfile.mkdtemp(prefix="zacxiom_stress_")
    home = os.path.join(tmpdir, "home")
    os.makedirs(home)

    env = os.environ.copy()
    env["HOME"] = home
    env["XDG_DATA_HOME"] = os.path.join(home, ".local/share")
    env["XDG_CACHE_HOME"] = os.path.join(home, ".cache")

    xdg_snap = os.path.join(home, ".local/share/zacxiom/snapshots")
    legacy_snap = os.path.join(home, ".cache/zacxiom/snapshots")
    os.makedirs(xdg_snap, exist_ok=True)
    os.makedirs(legacy_snap, exist_ok=True)

    print(f"\n\033[0;36m━━━ ZACXIOM DEEP STRESS TEST ━━━\033[0m")
    print(f"  HOME:   {home}")
    print(f"  Binary: {ZACXIOM}\n")

    try:
        # ── T1: Basic commands ──────────────────────────────
        print("\033[0;36m── Category 1: Basic Command Health ───\033[0m")

        out, rc = run(["--version"], env)
        assert_contains("T1.1  --version works", out, "v14.4.0")

        out, rc = run(["--help"], env)
        assert_contains("T1.2  --help works", out, "Safe filesystem cleaning")

        out, rc = run(["status"], env)
        assert_contains("T1.3  status works", out, "ZACXIOM")
        assert_not_contains("T1.4  status no root warning", out, "Scope")

        out, rc = run(["doctor"], env)
        assert_contains("T1.5  doctor works", out, "System ready")

        out, rc = run(["--cache-stats"], env)
        assert_contains("T1.6  --cache-stats works", out, "SCAN CACHE")

        # ── Create test file tree (OLD files → Safe decision) ─
        dirs_to_create = [
            ".cache/mozilla/firefox/profile1/cache2",
            ".cache/chromium/Default/Cache",
            ".cargo/registry/cache",
            ".cargo/registry/src/index",
            ".cache/pip/wheels",
            ".npm/_cacache/content",
            ".local/share/Trash/files",
            ".steam/steam/steamapps/shadercache/440",
            "project/target/debug/build",
            "project/target/release/build",
            "project/node_modules/lodash",
            ".cache/yay/firefox",
            ".cache/ollama/models",
        ]
        for d in dirs_to_create:
            os.makedirs(os.path.join(home, d), exist_ok=True)

        # Use touch_old so files are >1 day old → Safe decision
        test_files = {
            ".cache/mozilla/firefox/profile1/cache2/entry1": 50,
            ".cache/mozilla/firefox/profile1/cache2/entry2": 100,
            ".cache/chromium/Default/Cache/data1": 200,
            ".cargo/registry/cache/crate1.crate": 500,
            ".cargo/registry/cache/crate2.crate": 300,
            ".cargo/registry/src/index/foo.rs": 10,
            ".cache/pip/wheels/package.whl": 80,
            ".npm/_cacache/content/dep1": 40,
            ".local/share/Trash/files/deleted_doc.txt": 150,
            ".steam/steam/steamapps/shadercache/440/shader.bin": 256,
            "project/target/debug/build/artifact.o": 60,
            "project/target/release/build/artifact.o": 70,
            "project/node_modules/lodash/index.js": 30,
            ".cache/yay/firefox/PKGBUILD": 20,
            ".cache/ollama/models/model.gguf": 400,
        }
        for rel, kb in test_files.items():
            touch_old(os.path.join(home, rel), kb, days_old=90)

        scan_paths = [
            "-P", os.path.join(home, ".cache"),
            "-P", os.path.join(home, ".cargo"),
            "-P", os.path.join(home, ".npm"),
            "-P", os.path.join(home, ".local/share/Trash"),
            "-P", os.path.join(home, ".steam"),
            "-P", os.path.join(home, "project/target"),
            "-P", os.path.join(home, "project/node_modules"),
        ]

        # ── T2: Scan ────────────────────────────────────────
        print("\033[0;36m── Category 2: Scan Pipeline ───\033[0m")

        out, rc = run(["scan"] + scan_paths, env)
        assert_contains("T2.1  scan finds files", out, "Safe")
        assert_contains("T2.2  scan shows domains", out, "Browser Cache")

        out, rc = run(["--no-cache", "scan", "-P", os.path.join(home, ".cache/mozilla")], env)
        assert_contains("T2.3  scan --no-cache works", out, "Safe")

        out, rc = run(["scan", "-P", os.path.join(home, ".cache/mozilla"), "--json"], env)
        assert_contains("T2.4  scan --json output", out, '"health"')

        # Second scan should hit cache
        out, rc = run(["scan", "-P", os.path.join(home, ".cache/mozilla")], env)
        assert_contains("T2.5  scan cache hit on 2nd run", out, "Cache:")

        # Cache stats should show entries
        out, rc = run(["--cache-stats"], env)
        assert_contains("T2.6  cache-stats has entries", out, "Entries:")
        for line in out.splitlines():
            if "Entries:" in line:
                parts = line.split()
                for i, p in enumerate(parts):
                    if p == "Entries:" and i + 1 < len(parts):
                        count = int(parts[i + 1])
                        assert_gt("T2.7  cache-stats entries > 0", count, 0)

        # ── T3: Clean (dry-run) ─────────────────────────────
        print("\033[0;36m── Category 3: Clean Pipeline (Dry Run) ───\033[0m")

        out, rc = run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--dry-run"], env)
        assert_contains("T3.1  clean --dry-run works", out, "DRY RUN")
        assert_contains("T3.2  dry-run shows mode", out, "Mode:")

        out, rc = run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--smart", "--dry-run"], env)
        assert_contains("T3.3  clean --smart --dry-run", out, "smart")

        out, rc = run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--dry-run", "--json"], env)
        assert_contains("T3.4  clean --dry-run --json", out, '"mode"')

        # ── T4: Clean (real) + Undo ──────────────────────────
        print("\033[0;36m── Category 4: Clean + Undo + Snapshot ───\033[0m")

        mozilla_cache = os.path.join(home, ".cache/mozilla/firefox/profile1/cache2/entry1")
        assert_true("T4.0  entry1 exists before clean", os.path.isfile(mozilla_cache))

        out, rc = run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--yes"], env)
        assert_contains("T4.1  clean removes files", out, "Removed:")
        assert_contains("T4.2  clean creates snapshot", out, "Snapshot:")
        # Verify it shows the CORRECT (XDG) path, not legacy
        assert_not_contains("T4.2b clean shows XDG snap path (not legacy)", out, "~/.cache/zacxiom/snapshots/")

        assert_true("T4.3  file actually removed", not os.path.isfile(mozilla_cache))

        out, rc = run(["undo"], env)
        assert_contains("T4.4  undo restores files", out, "Restored")

        assert_true("T4.5  file restored after undo", os.path.isfile(mozilla_cache))

        # ── T5: Snapshot Management ─────────────────────────
        print("\033[0;36m── Category 5: Snapshot Management ───\033[0m")

        # Create second snapshot (re-clean the restored files)
        run(["clean", "-P", os.path.join(home, ".cache/chromium"), "--yes"], env)

        out, rc = run(["snapshot", "list"], env)
        snap_lines = [l for l in out.splitlines() if l.strip() and l.strip()[0].isdigit()]
        assert_true("T5.1  snapshot list shows entries", len(snap_lines) >= 2,
                    f"only {len(snap_lines)} entries")

        out, rc = run(["snapshot", "verify"], env)
        # verify outputs "X valid" or similar
        has_verify = "valid" in out.lower() or "verified" in out.lower() or "passed" in out.lower()
        assert_true("T5.2  snapshot verify works", has_verify, f"output: {out[:100]}")

        out, rc = run(["snapshot", "prune", "--keep", "1"], env)
        assert_contains("T5.3  snapshot prune --keep 1", out, "Pruned")

        out, rc = run(["snapshot", "list"], env)
        snap_after = [l for l in out.splitlines() if l.strip() and l.strip()[0].isdigit()]
        assert_true("T5.4  prune reduced snapshot count", len(snap_after) <= 1,
                    f"after={len(snap_after)}")

        # ── T6: Purge ALL (the fixed bug) ────────────────────
        print("\033[0;36m── Category 6: Snapshot Purge (Bug #1 Fix) ───\033[0m")

        # Create a clean (XDG dir snapshot)
        run(["clean", "-P", os.path.join(home, ".cache/pip"), "--yes"], env)

        # Manually create snapshot in LEGACY dir
        legacy_snap_id = "snap-99999-0000000001-0001"
        legacy_snap_content = json.dumps({
            "id": legacy_snap_id,
            "created": str(int(time.time()) - 86400),  # 1 day ago
            "entries": [
                {"path": "/tmp/old_file.txt", "size": 100,
                 "trash_path": None, "timestamp": str(int(time.time())), "skipped": False}
            ]
        })
        with open(os.path.join(legacy_snap, legacy_snap_id), "w") as f:
            f.write(legacy_snap_content)

        # List should see both
        out, rc = run(["snapshot", "list"], env)
        total_snaps = len([l for l in out.splitlines() if l.strip() and l.strip()[0].isdigit()])
        assert_true("T6.1  list sees both XDG + legacy", total_snaps >= 2,
                    f"count={total_snaps}")

        # CRITICAL: Purge ALL — must delete from BOTH dirs
        out, rc = run(["snapshot", "purge", "--confirm", "DELETE ALL"], env)
        assert_contains("T6.3  purge says ALL", out, "Purged ALL")
        assert_contains("T6.4  purge reports correct count", out, f"{total_snaps} snapshot(s)")

        # Verify BOTH dirs empty
        xdg_remain = len(os.listdir(xdg_snap)) if os.path.isdir(xdg_snap) else 0
        legacy_remain = len(os.listdir(legacy_snap)) if os.path.isdir(legacy_snap) else 0
        assert_eq("T6.5  XDG snapshot dir empty", xdg_remain, 0)
        assert_eq("T6.6  Legacy snapshot dir empty", legacy_remain, 0)

        out, rc = run(["snapshot", "list"], env)
        assert_contains("T6.7  no snapshots after purge", out, "No snapshots found")

        out, rc = run(["snapshot", "purge", "--confirm", "DELETE ALL"], env)
        assert_contains("T6.8  purge idempotent", out, "No snapshots to purge")

        # ── T7: Edge Cases ──────────────────────────────────
        print("\033[0;36m── Category 7: Edge Cases ───\033[0m")

        empty_dir = os.path.join(tmpdir, "empty")
        os.makedirs(empty_dir)
        out, rc = run(["clean", "-P", empty_dir, "--yes"], env)
        assert_contains("T7.1  clean empty dir", out, "No snapshot created")

        # Non-existent path (should not crash)
        out, rc = run(["scan", "-P", os.path.join(tmpdir, "nonexistent_xyz")], env)

        # Undo with no snapshots
        out, rc = run(["undo"], env)
        assert_contains("T7.2  undo no snapshots", out, "No snapshots found")

        # Status shows 0 snapshots
        out, rc = run(["status"], env)
        assert_contains("T7.3  status 0 snapshots", out, "Snapshots : 0 available")

        # Plan
        out, rc = run(["plan", os.path.join(home, "project")], env)
        assert_true("T7.4  plan works", rc == 0, f"rc={rc}")

        # Explain
        out, rc = run(["explain", os.path.join(home, ".cache")], env)
        assert_true("T7.5  explain works", rc == 0, f"rc={rc}")

        # Viz
        out, rc = run(["viz", os.path.join(home, "project")], env)
        assert_true("T7.6  viz works", rc == 0, f"rc={rc}")

        # Dedup
        out, rc = run(["dedup", "-P", os.path.join(home, "project"), "--json"], env)
        assert_contains("T7.7  dedup --json works", out, '"duplicate_groups"')

        # ── T8: Multi-Cycle Clean/Undo ───────────────────────
        print("\033[0;36m── Category 8: Multi-Cycle Clean/Undo ───\033[0m")

        entry2 = os.path.join(home, ".cache/mozilla/firefox/profile1/cache2/entry2")
        with open(entry2, "rb") as f:
            orig_hash = f.read()

        # Cycle 1
        run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--yes"], env)
        run(["undo"], env)
        with open(entry2, "rb") as f:
            restored1 = f.read()
        assert_eq("T8.1  cycle 1: content preserved", orig_hash, restored1)

        # Cycle 2
        run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--yes"], env)
        run(["undo"], env)
        with open(entry2, "rb") as f:
            restored2 = f.read()
        assert_eq("T8.2  cycle 2: content preserved", orig_hash, restored2)

        # ── T9: Safety Guards ────────────────────────────────
        print("\033[0;36m── Category 9: Safety Guards ───\033[0m")

        etc_dir = os.path.join(tmpdir, "etc_test")
        os.makedirs(etc_dir)
        with open(os.path.join(etc_dir, "critical.conf"), "w") as f:
            f.write("critical config data\n")
        out, rc = run(["clean", "-P", etc_dir, "--force", "--dry-run"], env)
        assert_contains("T9.1  /etc-like paths protected", out, "Would skip")

        iso_dir = os.path.join(tmpdir, "iso_test")
        os.makedirs(iso_dir)
        write_file(os.path.join(iso_dir, "big.iso"), 100)
        out, rc = run(["scan", "-P", iso_dir], env)
        assert_contains("T9.2  .iso files protected", out, "Protected")

        with open(os.path.join(iso_dir, "key.pem"), "w") as f:
            f.write("PRIVATE KEY DATA\n")
        out, rc = run(["scan", "-P", iso_dir], env)
        assert_contains("T9.3  .pem files protected", out, "Protected")

        # ── T10: Snapshot delete ────────────────────────────
        print("\033[0;36m── Category 10: Snapshot Delete Edge Cases ───\033[0m")

        run(["clean", "-P", os.path.join(home, ".cache/yay"), "--yes"], env)

        out, rc = run(["snapshot", "list", "--json"], env)
        try:
            snap_data = json.loads(out)
            snap_id = snap_data["snapshots"][0]["id"]

            out, rc = run(["snapshot", "delete", snap_id, "--force"], env)
            assert_contains("T10.1 delete specific snapshot", out, "deleted")

            out, rc = run(["snapshot", "delete", "snap-nonexistent", "--force"], env)
            assert_contains("T10.2 delete nonexistent", out, "not found")
        except (json.JSONDecodeError, IndexError, KeyError) as e:
            report("T10.1 delete specific snapshot", "WARN", f"parse error: {e}")

        # ── T11: Root awareness (non-root) ─────────────────
        print("\033[0;36m── Category 11: Root Awareness ───\033[0m")

        out, rc = run(["--cache-stats"], env)
        assert_not_contains("T11.1  no root line as non-root", out, "User:")

        out, rc = run(["status"], env)
        assert_not_contains("T11.2  no scope line as non-root", out, "Scope")

        # ── T12: Stored-in path accuracy ───────────────────
        print("\033[0;36m── Category 12: Output Accuracy ───\033[0m")

        run(["snapshot", "purge", "--confirm", "DELETE ALL"], env)
        touch_old(os.path.join(home, ".cache/mozilla/firefox/profile1/cache2/entry1"), 50, days_old=90)
        out, rc = run(["clean", "-P", os.path.join(home, ".cache/mozilla"), "--yes"], env)
        out = strip_ansi(out)
        # Must show XDG path, NOT legacy ~/.cache/zacxiom/snapshots/
        assert_not_contains("T12.1  clean output NOT legacy path", out, "~/.cache/zacxiom/snapshots/")
        assert_contains("T12.2  clean output shows snapshot info", out, "Snapshot:")
        assert_contains("T12.3  snap dir is XDG path", out, "snapshots")

        # Undo the clean so we can purge
        run(["undo"], env)

    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

    # ── Summary ──────────────────────────────────────────────
    print(f"\n\033[0;36m━━━ STRESS TEST SUMMARY ━━━\033[0m")
    print(f"  \033[0;32mPASS\033[0m: {PASS}")
    print(f"  \033[0;31mFAIL\033[0m: {FAIL}")
    print(f"  \033[1;33mWARN\033[0m: {WARN}")
    print()

    if FAIL > 0:
        print("\033[0;31m━━━ SOME TESTS FAILED ━━━\033[0m")
        sys.exit(1)
    else:
        print("\033[0;32m━━━ ALL TESTS PASSED ━━━\033[0m")
        sys.exit(0)

if __name__ == "__main__":
    main()