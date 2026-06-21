#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# install.sh - Build and install zacxiom for the current user.
# Usage: ./scripts/install.sh
set -euo pipefail

cd "$(dirname "$0")/.."

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="${PREFIX}/bin"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}━━━ zacxiom install ━━━${NC}"
echo ""

# Ensure cargo is available
if ! command -v cargo &>/dev/null; then
  if [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
  else
    echo -e "${RED}Error: cargo not found. Install Rust: https://rustup.rs${NC}"
    exit 1
  fi
fi

# Build release
echo "Building release binary..."
cargo build --release --locked
echo -e "  ${GREEN}✓${NC} Build complete"
echo ""

# Install binary without privilege escalation.
echo "Installing to ${BIN_DIR}..."
mkdir -p "$BIN_DIR"
install -m 755 target/release/zacxiom "$BIN_DIR/zacxiom"
echo -e "  ${GREEN}✓${NC} Installed: ${BIN_DIR}/zacxiom"
echo ""

# Set up config directory
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/zacxiom"
mkdir -p "$CONFIG_DIR"
echo "Config directory: $CONFIG_DIR"

# Create default policy if not exists
if [ ! -f "$CONFIG_DIR/policy.json" ]; then
  cat > "$CONFIG_DIR/policy.json" << 'EOF'
{
  "protected_paths": [],
  "max_file_size": 0,
  "skip_domains": [],
  "min_risk_for_clean": 0.0
}
EOF
  echo -e "  ${GREEN}✓${NC} Default policy created"
fi

# Create cache directory
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/zacxiom"
mkdir -p "$CACHE_DIR"

echo ""
echo -e "${GREEN}━━━ zacxiom installed ━━━${NC}"
echo "  Binary : ${BIN_DIR}/zacxiom"
echo "  Config : ${CONFIG_DIR}"
echo "  Cache  : ${CACHE_DIR}"
echo ""
echo "  Verify: zacxiom -V"
echo "  Usage : zacxiom --help"
