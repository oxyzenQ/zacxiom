#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

set -euo pipefail

case "${1:---check-all}" in
  --check-all|check-all) ;;
  *)
    echo "Usage: ./scripts/build.sh --check-all" >&2
    exit 2
    ;;
esac

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# ensure cargo in PATH
if ! command -v cargo &>/dev/null; then
  . "$HOME/.cargo/env" 2>/dev/null || true
fi

cd "$(dirname "$0")/.."

echo -e "${YELLOW}━━━ ZACXIOM CHECK-ALL ━━━${NC}"

check_step() {
  local label="$1"
  shift
  echo -n "  $label ... "
  if "$@" >/dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
  else
    echo -e "${RED}FAIL${NC}"
    echo "  --- rerun with output: $*"
    return 1
  fi
}

FAILED=0

check_step "fmt       " cargo fmt --all -- --check || FAILED=1
check_step "clippy    " cargo clippy --all-targets --all-features -- -D warnings || FAILED=1
check_step "build     " cargo build || FAILED=1
check_step "test      " cargo test || FAILED=1
if command -v cargo-deny >/dev/null 2>&1; then
  check_step "deny      " cargo deny check || FAILED=1
elif cargo audit --version >/dev/null 2>&1; then
  check_step "audit     " cargo audit || FAILED=1
else
  echo -e "  audit      ... ${YELLOW}SKIP${NC} (install cargo-deny or cargo-audit)"
fi

echo ""
if [ "$FAILED" -eq 0 ]; then
  echo -e "${GREEN}━━━ ALL CHECKS PASSED ━━━${NC}"
else
  echo -e "${RED}━━━ SOME CHECKS FAILED ━━━${NC}"
  exit 1
fi
