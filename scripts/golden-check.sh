#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# golden-check.sh — Verify CLI output hasn't changed unexpectedly.
# Run as part of CI to catch unintended output changes.
# Usage: ./scripts/golden-check.sh
set -euo pipefail

cd "$(dirname "$0")/.."

GOLDEN_DIR="tests/golden"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}━━━ golden check ━━━${NC}"

# Ensure binary exists
if [ ! -f target/release/zacxiom ]; then
    echo -e "  ${RED}✗${NC} target/release/zacxiom not found — build first"
    exit 1
fi

ZACXIOM=target/release/zacxiom

FAIL=0
PASS=0

check_golden() {
    local name="$1"
    shift
    echo -n "  ${CYAN}[....]${NC} ${name} ... "
    "$@" > "$TMPDIR/${name}.txt" 2>&1
    if diff -q "$GOLDEN_DIR/${name}.txt" "$TMPDIR/${name}.txt" > /dev/null 2>&1; then
        echo -e "\r  ${GREEN}[PASS]${NC} ${name}"
        PASS=$((PASS + 1))
    else
        echo -e "\r  ${RED}[DIFF]${NC} ${name}"
        echo "    --- expected"
        echo "    +++ actual"
        diff "$GOLDEN_DIR/${name}.txt" "$TMPDIR/${name}.txt" | head -20 | sed 's/^/    /'
        FAIL=$((FAIL + 1))
    fi
}

check_golden "help"   "$ZACXIOM" help
check_golden "status" "$ZACXIOM" status
check_golden "doctor" "$ZACXIOM" doctor

echo ""
echo -e "  ${GREEN}Passed: ${PASS}${NC}  ${RED}Failed: ${FAIL}${NC}"

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo -e "  ${RED}❌ Golden test failed. If output changed intentionally, regenerate:${NC}"
    echo "     ./scripts/golden-update.sh"
    exit 1
else
    echo -e "  ${GREEN}✅ Golden tests passed.${NC}"
fi
