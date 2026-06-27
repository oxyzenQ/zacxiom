#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# gatekeeper.sh — Pre-commit quality gate. Run before pushing.
# Usage: ./scripts/gatekeeper.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# Ensure cargo is available
if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

export RUST_BACKTRACE=full

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0

check() {
    local name="$1"
    shift
    echo -n "  ${CYAN}[....]${NC} ${name} ... "
    local out
    if out=$("$@" 2>&1); then
        echo -e "\r  ${GREEN}[PASS]${NC} ${name}"
        PASS=$((PASS + 1))
    else
        echo -e "\r  ${RED}[FAIL]${NC} ${name}"
        echo "$out" | tail -5 | sed 's/^/        /'
        FAIL=$((FAIL + 1))
    fi
}

echo -e "${CYAN}━━━ zacxiom gatekeeper ━━━${NC}"
echo ""

# ── Format + Lint ──
check "cargo fmt --check"           cargo fmt --check
check "cargo clippy"                cargo clippy --all-targets --all-features -- -D warnings
check "cargo test"                  cargo test -- --test-threads=1
check "cargo build --release"       cargo build --release --locked

# ── Security ──
if command -v cargo-audit &>/dev/null; then
    check "cargo audit"             cargo audit
else
    echo -e "  ${YELLOW}[SKIP]${NC} cargo audit (not installed)"
fi

if command -v cargo-deny &>/dev/null; then
    check "cargo deny check"        cargo deny check
else
    echo -e "  ${YELLOW}[SKIP]${NC} cargo deny (not installed)"
fi

# ── Spelling ──
if command -v codespell &>/dev/null; then
    if [ -f .codespellrc ]; then
        check "codespell"           codespell --config .codespellrc .
    else
        check "codespell"           codespell .
    fi
else
    echo -e "  ${YELLOW}[SKIP]${NC} codespell (not installed)"
fi

# ── Docs ──
check "README exists"               test -f README.md
check "LICENSE exists"              test -f LICENSE
check "TRADEMARK exists"            test -f TRADEMARK.md

echo ""
echo -e "  ${GREEN}Passed: ${PASS}${NC}  ${RED}Failed: ${FAIL}${NC}"

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo -e "  ${RED}❌ Commit blocked — fix failures above.${NC}"
    exit 1
else
    echo -e "  ${GREEN}✅ All gates passed.${NC}"
fi
