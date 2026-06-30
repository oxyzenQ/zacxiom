#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# install.sh - Build and install zacxiom.
#
# Usage:
#   ./scripts/install.sh           # user install → ~/.local/bin
#   ./scripts/install.sh --system  # system install → /usr/local/bin (needs sudo for copy only)
set -euo pipefail

cd "$(dirname "$0")/.."

SYSTEM_MODE=false
if [[ "${1:-}" == "--system" ]]; then
  SYSTEM_MODE=true
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

if $SYSTEM_MODE; then
  echo -e "${CYAN}━━━ zacxiom install (system) ━━━${NC}"
else
  echo -e "${CYAN}━━━ zacxiom install (user) ━━━${NC}"
fi
echo ""

# ── Step 1: Build (ALWAYS as user, NEVER with sudo) ──
# Ensure cargo is available in the current user's environment
if ! command -v cargo &>/dev/null; then
  if [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
  else
    echo -e "${RED}Error: cargo not found. Install Rust: https://rustup.rs${NC}"
    exit 1
  fi
fi

echo "Building release binary..."
cargo build --release --locked
echo -e "  ${GREEN}✓${NC} Build complete"
echo ""

# ── Step 2: Install ──
if $SYSTEM_MODE; then
  BIN_DIR="/usr/local/bin"
  echo "Need root to install into ${BIN_DIR}"

  if command -v sudo &>/dev/null; then
    echo -e "  ${YELLOW}→ sudo install -Dm755 target/release/zacxiom ${BIN_DIR}/zacxiom${NC}"
    sudo install -Dm755 target/release/zacxiom "$BIN_DIR/zacxiom"
  else
    echo -e "${RED}Error: sudo not found. Run as root or install without --system.${NC}"
    exit 1
  fi
else
  BIN_DIR="${PREFIX:-$HOME/.local}/bin"
  echo "Installing to ${BIN_DIR}..."
  mkdir -p "$BIN_DIR"
  install -m 755 target/release/zacxiom "$BIN_DIR/zacxiom"
fi
echo -e "  ${GREEN}✓${NC} Installed: ${BIN_DIR}/zacxiom"
echo ""

# ── Step 3: Config + cache (user dirs, even for system install) ──
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/zacxiom"
mkdir -p "$CONFIG_DIR"

# v13: Install example config.toml if user doesn't have one
EXAMPLE_CONFIG="$(pwd)/example/config.toml"
if [ -f "$EXAMPLE_CONFIG" ]; then
  if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cp "$EXAMPLE_CONFIG" "$CONFIG_DIR/config.toml"
    chmod 644 "$CONFIG_DIR/config.toml"
    echo -e "  ${GREEN}✓${NC} Default config.toml installed to $CONFIG_DIR/config.toml"
    echo -e "    Edit with: nano $CONFIG_DIR/config.toml"
    echo -e "    Validate with: zacxiom --testconf"
  else
    echo -e "  ${YELLOW}ℹ${NC} config.toml already exists — keeping your customizations"
    echo -e "    Example config available at: example/config.toml"
    echo -e "    To reset: rm $CONFIG_DIR/config.toml && ./scripts/install.sh"
  fi
else
  echo -e "  ${YELLOW}⚠${NC} example/config.toml not found — skipping config install"
fi

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

CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/zacxiom"
mkdir -p "$CACHE_DIR"
echo "Config directory: $CONFIG_DIR"
echo "Cache directory:  $CACHE_DIR"

echo ""
echo -e "${GREEN}━━━ zacxiom installed ━━━${NC}"
echo "  Binary : ${BIN_DIR}/zacxiom"
echo "  Config : ${CONFIG_DIR}"
echo "  Cache  : ${CACHE_DIR}"
echo ""

# ── Step 4: Auto-validate ──
BIN="${BIN_DIR}/zacxiom"
echo -e "${CYAN}Validating installation...${NC}"

if [ -x "${BIN}" ]; then
  echo -e "  ${GREEN}✓${NC} Binary executable"
else
  echo -e "  ${RED}✗${NC} Binary not executable"
  exit 1
fi

# PATH check
if command -v zacxiom >/dev/null 2>&1; then
  echo -e "  ${GREEN}✓${NC} PATH detected"
elif [ -x "${HOME}/.local/bin/zacxiom" ]; then
  echo -e "  ${YELLOW}⚠${NC} PATH not detected — add ~/.local/bin to your PATH"
else
  echo -e "  ${YELLOW}⚠${NC} PATH check skipped"
fi

# Version check
INSTALLED_VERSION=$("${BIN}" -V 2>/dev/null | grep -oP '[0-9]+\.[0-9]+\.[0-9]+' || echo "unknown")
if [ "${INSTALLED_VERSION}" != "unknown" ]; then
  echo -e "  ${GREEN}✓${NC} Version verified: v${INSTALLED_VERSION}"
else
  echo -e "  ${YELLOW}⚠${NC} Could not verify version"
fi

echo ""
echo "  Usage : zacxiom --help"

if ! $SYSTEM_MODE; then
  echo ""
  echo "  💡 Tip: for system-wide install, use:"
  echo "     ./scripts/install.sh --system"
fi

echo ""
echo -e "  ${CYAN}Next steps:${NC}"
echo "    zacxiom scan       # inspect your system (safe, read-only)"
echo "    zacxiom plan       # see what could be cleaned (defaults to HOME)"
echo "    zacxiom clean      # clean only safe files"
echo ""
echo "  No files are deleted until you run 'zacxiom clean'."
