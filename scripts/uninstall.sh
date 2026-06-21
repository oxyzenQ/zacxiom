#!/usr/bin/env bash
# uninstall.sh — Remove zacxiom from the system
# Usage: ./scripts/uninstall.sh [--prefix /usr/local]
set -euo pipefail

cd "$(dirname "$0")/.."

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}━━━ zacxiom uninstall ━━━${NC}"
echo ""

# Remove binary
if [ -f "$BIN_DIR/zacxiom" ]; then
  rm -f "$BIN_DIR/zacxiom"
  echo -e "  ${GREEN}✓${NC} Removed: ${BIN_DIR}/zacxiom"
else
  echo -e "  ${YELLOW}⚠${NC} Binary not found: ${BIN_DIR}/zacxiom"
fi

# Ask about config/cache
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
