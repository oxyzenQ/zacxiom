#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# uninstall.sh - Remove zacxiom.
#
# Usage:
#   ./scripts/uninstall.sh           # remove user install from ~/.local/bin
#   ./scripts/uninstall.sh --system  # remove system install from /usr/local/bin (needs sudo)
set -euo pipefail

cd "$(dirname "$0")/.."

SYSTEM_MODE=false
if [[ "${1:-}" == "--system" ]]; then
  SYSTEM_MODE=true
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

if $SYSTEM_MODE; then
  echo -e "${CYAN}━━━ zacxiom uninstall (system) ━━━${NC}"
  BIN_DIR="/usr/local/bin"
else
  echo -e "${CYAN}━━━ zacxiom uninstall (user) ━━━${NC}"
  BIN_DIR="${PREFIX:-$HOME/.local}/bin"
fi
echo ""

# Remove binary
if [ -f "$BIN_DIR/zacxiom" ]; then
  if $SYSTEM_MODE; then
    sudo rm -f "$BIN_DIR/zacxiom"
  else
    rm -f "$BIN_DIR/zacxiom"
  fi
  echo -e "  ${GREEN}✓${NC} Removed: ${BIN_DIR}/zacxiom"
else
  echo -e "  ${YELLOW}⚠${NC} Binary not found: ${BIN_DIR}/zacxiom"
fi

# Ask about config/cache (always in user dirs)
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/zacxiom"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/zacxiom"

echo ""
echo "Config directory: $CONFIG_DIR"
echo "Cache directory : $CACHE_DIR"
echo ""
read -r -p "Remove config and cache directories? [y/N] " yn
case $yn in
  [Yy]*)
    rm -rf "$CONFIG_DIR"
    rm -rf "$CACHE_DIR"
    echo -e "  ${GREEN}✓${NC} Config and cache removed"
    ;;
  *)
    echo "  Skipped — config and cache preserved"
    ;;
esac

echo ""
echo -e "${GREEN}━━━ zacxiom uninstalled ━━━${NC}"
