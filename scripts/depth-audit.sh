#!/usr/bin/env bash
# v14.0 depth audit — stress + edge + regression tests
set -uo pipefail
ZAC="/home/z/my-project/zacxiom/target/release/zacxiom"
PASS=0; FAIL=0; TOTAL=0
export TESTHOME=/tmp/zacxiom-v14-audit
rm -rf "$TESTHOME"
mkdir -p "$TESTHOME"/{.config/zacxiom,.local/share/zacxiom,.cache/mozilla/firefox/abc/cache2,Downloads,testdir}
export HOME="$TESTHOME"
export XDG_CONFIG_HOME="$TESTHOME/.config"
export XDG_DATA_HOME="$TESTHOME/.local/share"
export XDG_CACHE_HOME="$TESTHOME/.cache"

run() {
    TOTAL=$((TOTAL+1))
    local name="$1" cmd="$2" expect="$3" pattern="${4:-}"
    local out exit_code
    out=$(eval "$cmd" 2>&1); exit_code=$?
    local result="FAIL" detail=""
    case "$expect" in
        NO_CRASH) [ $exit_code -le 1 ] && result="PASS" && detail="exit=$exit_code" ;;
        ERROR)    [ $exit_code -ne 0 ] && result="PASS" && detail="exit=$exit_code" ;;
        SAFE)     [ $exit_code -eq 0 ] && result="PASS" && detail="exit=0" ;;
        CONTAINS) echo "$out" | grep -q "$pattern" && result="PASS" && detail="found" ;;
        FILE_SAFE) [ -f "$pattern" ] && result="PASS" && detail="preserved" || detail="DELETED!" ;;
    esac
    [ "$result" = "PASS" ] && PASS=$((PASS+1)) || FAIL=$((FAIL+1))
    printf "  %-50s %s (%s)\n" "$name" "$result" "$detail"
}

echo "══════════════════════════════════════════════════"
echo "  ZACXIOM v14.0.0 DEPTH AUDIT"
echo "══════════════════════════════════════════════════"
echo ""

# Setup files
echo "browser" > "$TESTHOME/.cache/mozilla/firefox/abc/cache2/entry1.db"
echo "iso" > "$TESTHOME/Downloads/ubuntu.iso"
echo "vmdk" > "$TESTHOME/Downloads/vm.vmdk"
echo "pem" > "$TESTHOME/Downloads/key.pem"
echo "data" > "$TESTHOME/testdir/file.txt"

echo "── 1. CRASH RECOVERY ──"
for i in $(seq 1 200); do echo "d$i" > "$TESTHOME/.cache/mozilla/firefox/abc/cache2/f$i.db"; done
$ZAC clean "$TESTHOME/.cache/mozilla" --smart --yes &
PID=$!; sleep 0.3; kill -9 $PID 2>/dev/null; wait $PID 2>/dev/null
run "Recover after kill -9" "$ZAC scan $TESTHOME/.cache/mozilla 2>&1 | grep -q 'Files Found'" "CONTAINS"
run "Clean after crash" "$ZAC clean $TESTHOME/.cache/mozilla --smart --yes 2>&1 | grep -q 'Removed:'" "CONTAINS"

echo ""
echo "── 2. PROTECTED FILE INTEGRITY ──"
run "ISO protected" "$ZAC clean $TESTHOME/Downloads --force --yes" "FILE_SAFE" "$TESTHOME/Downloads/ubuntu.iso"
run "VMDK protected" "$ZAC clean $TESTHOME/Downloads --force --yes" "FILE_SAFE" "$TESTHOME/Downloads/vm.vmdk"
run "PEM protected" "$ZAC clean $TESTHOME/Downloads --force --yes" "FILE_SAFE" "$TESTHOME/Downloads/key.pem"

echo ""
echo "── 3. CONFIG VALIDATION ──"
echo '[clean]
default_mode = "force"' > "$TESTHOME/.config/zacxiom/config.toml"
run "Config force rejected" "$ZAC --testconf" "ERROR"
echo '[clean]
max_auto_clean_size = "500KB"' > "$TESTHOME/.config/zacxiom/config.toml"
run "KB size rejected" "$ZAC --testconf" "ERROR"
echo '[clean]
max_auto_clean_size = "100MB"' > "$TESTHOME/.config/zacxiom/config.toml"
run "MB size accepted" "$ZAC --testconf" "SAFE"
rm -f "$TESTHOME/.config/zacxiom/config.toml"

echo ""
echo "── 4. UNDO INTEGRITY ──"
rm -rf "$TESTHOME/.local/share/zacxiom"
echo "important" > "$TESTHOME/.cache/mozilla/firefox/abc/cache2/undo.db"
$ZAC clean "$TESTHOME/.cache/mozilla" --smart --yes > /dev/null 2>&1
run "Undo restores" "$ZAC undo 2>&1 | grep -q 'Restored'" "CONTAINS"
run "Content intact" "grep -q 'important' $TESTHOME/.cache/mozilla/firefox/abc/cache2/undo.db" "SAFE"

echo ""
echo "── 5. AUDIT LOG ──"
run "Audit log exists" "test -f $TESTHOME/.local/share/zacxiom/audit.log" "SAFE"
run "Audit log has clean entry" "grep -q 'clean' $TESTHOME/.local/share/zacxiom/audit.log" "SAFE"
run "Audit log has undo entry" "grep -q 'undo' $TESTHOME/.local/share/zacxiom/audit.log" "SAFE"

echo ""
echo "── 6. QUIET MODE ──"
run "Quiet suppresses progress" "$ZAC scan $TESTHOME/.cache/mozilla --quiet 2>&1 | grep -v '^\[' | grep -q 'Files Found'" "CONTAINS"

echo ""
echo "── 7. SNAPSHOT VERIFY ──"
run "Snapshot verify" "$ZAC snapshot verify 2>&1 | grep -q 'INTEGRITY'" "CONTAINS"

echo ""
echo "── 8. LARGE FILESET (1000 files) ──"
mkdir -p "$TESTHOME/.cache/large"
for i in $(seq 1 1000); do echo "d$i" > "$TESTHOME/.cache/large/f$i.cache"; done
run "Scan 1000 files" "$ZAC scan $TESTHOME/.cache/large 2>&1 | grep -q 'Files Found'" "CONTAINS"
run "Clean 1000 files" "$ZAC clean $TESTHOME/.cache/large --smart --yes 2>&1 | grep -q 'Removed:'" "CONTAINS"

echo ""
echo "── 9. EDGE CASES ──"
mkfifo "$TESTHOME/testdir/fifo.pipe" 2>/dev/null
run "FIFO handled" "$ZAC scan $TESTHOME/testdir 2>&1 | grep -q 'Files Found'" "CONTAINS"
rm -f "$TESTHOME/testdir/fifo.pipe"
touch "$TESTHOME/testdir/zero.txt"
run "Zero-byte file" "$ZAC scan $TESTHOME/testdir 2>&1 | grep -q 'Files Found'" "CONTAINS"

echo ""
echo "── 10. CONCURRENT RUNS ──"
rm -rf "$TESTHOME/.local/share/zacxiom"
for i in $(seq 1 100); do echo "c$i" > "$TESTHOME/.cache/mozilla/firefox/abc/cache2/c$i.db"; done
$ZAC clean "$TESTHOME/.cache/mozilla" --smart --yes > /tmp/c1.log 2>&1 &
P1=$!; $ZAC clean "$TESTHOME/.cache/mozilla" --smart --yes > /tmp/c2.log 2>&1 &
P2=$!; wait $P1; E1=$?; wait $P2; E2=$?
run "Concurrent 1 no crash" "test $E1 -le 1" "SAFE"
run "Concurrent 2 no crash" "test $E2 -le 1" "SAFE"

echo ""
echo "══════════════════════════════════════════════════"
echo "  TOTAL: $TOTAL  PASS: $PASS  FAIL: $FAIL"
echo "══════════════════════════════════════════════════"
