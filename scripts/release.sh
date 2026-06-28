#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# release.sh — Generate versioned release archive and checksum.
# Usage: ./scripts/release.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# Extract version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH="linux-amd64"
RELEASE_NAME="zacxiom-v${VERSION}-${ARCH}"
OUT_DIR="target/dist"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}━━━ zacxiom release v${VERSION} ━━━${NC}"
echo ""

# Ensure release binary exists
if [ ! -f target/release/zacxiom ]; then
    echo -e "${RED}Error: target/release/zacxiom not found. Run 'cargo build --release --locked' first.${NC}"
    exit 1
fi

# Prepare output directory
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

# Copy release artifacts
echo "Packaging ${RELEASE_NAME}..."
cp target/release/zacxiom "$OUT_DIR/"
cp README.md "$OUT_DIR/"
cp LICENSE "$OUT_DIR/"

# Create archive (write to target/ so tar doesn't see the archive in OUT_DIR)
ARCHIVE="${RELEASE_NAME}.tar.gz"
tar -czf "target/${ARCHIVE}" -C "$OUT_DIR" .

# Generate SHA-512 checksum
(cd target && sha512sum "${ARCHIVE}" > "${ARCHIVE}.sha512sum")
echo -e "  ${GREEN}✓${NC} Archive: target/${ARCHIVE}"
echo -e "  ${GREEN}✓${NC} Checksum: target/${ARCHIVE}.sha512sum"
echo ""

# Verify checksum
echo "Verifying checksum..."
(cd target && sha512sum -c "${ARCHIVE}.sha512sum")
echo -e "  ${GREEN}✓${NC} Checksum verified"
echo ""

# Display archive contents
echo "Archive contents:"
tar -tzf "target/${ARCHIVE}" | while read -r f; do
    echo "  $f"
done

echo ""
echo -e "${GREEN}━━━ release ready ━━━${NC}"
echo "  Archive:  target/${ARCHIVE}"
echo "  Checksum: target/${ARCHIVE}.sha512sum"
echo ""
echo "  Next: create GitHub Release and upload both files"
