#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# release.sh — Generate versioned release archives (gnu + musl) and checksums.
# v14.0: Builds two amd64 binaries — glibc (gnu) and static (musl).
# Usage: ./scripts/release.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# Extract version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH="linux-amd64"
RELEASE_GNU="zacxiom-v${VERSION}-${ARCH}-gnu"
RELEASE_MUSL="zacxiom-v${VERSION}-${ARCH}-musl"
OUT_DIR="target/dist"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36c'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${CYAN}━━━ zacxiom release v${VERSION} ━━━${NC}"
echo ""

# ── Build glibc (gnu) binary ──
echo -e "${YELLOW}Building glibc (gnu) binary...${NC}"
cargo build --release --locked
echo -e "  ${GREEN}✓${NC} target/release/zacxiom"

# ── Build static musl binary ──
echo ""
echo -e "${YELLOW}Building static musl binary...${NC}"
if rustup target list --installed 2>/dev/null | grep -q x86_64-unknown-linux-musl; then
    cargo build --release --locked --target x86_64-unknown-linux-musl
    MUSL_BIN="target/x86_64-unknown-linux-musl/release/zacxiom"
    echo -e "  ${GREEN}✓${NC} ${MUSL_BIN}"
else
    echo -e "  ${YELLOW}⚠${NC} musl target not installed. Install with:"
    echo -e "     rustup target add x86_64-unknown-linux-musl"
    echo -e "  ${YELLOW}⚠${NC} Skipping musl build. Only gnu binary will be packaged."
    MUSL_BIN=""
fi

# ── Prepare output directory ──
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

# ── Package gnu binary ──
echo ""
echo "Packaging ${RELEASE_GNU}..."
mkdir -p "$OUT_DIR/${RELEASE_GNU}"
cp target/release/zacxiom "$OUT_DIR/${RELEASE_GNU}/"
cp README.md "$OUT_DIR/${RELEASE_GNU}/"
cp LICENSE "$OUT_DIR/${RELEASE_GNU}/"
ARCHIVE_GNU="${RELEASE_GNU}.tar.gz"
tar -czf "target/${ARCHIVE_GNU}" -C "$OUT_DIR" "${RELEASE_GNU}"
(cd target && sha512sum "${ARCHIVE_GNU}" > "${ARCHIVE_GNU}.sha512sum")
echo -e "  ${GREEN}✓${NC} Archive: target/${ARCHIVE_GNU}"
echo -e "  ${GREEN}✓${NC} Checksum: target/${ARCHIVE_GNU}.sha512sum"

# ── Package musl binary (if built) ──
if [ -n "$MUSL_BIN" ]; then
    echo ""
    echo "Packaging ${RELEASE_MUSL}..."
    mkdir -p "$OUT_DIR/${RELEASE_MUSL}"
    cp "$MUSL_BIN" "$OUT_DIR/${RELEASE_MUSL}/"
    cp README.md "$OUT_DIR/${RELEASE_MUSL}/"
    cp LICENSE "$OUT_DIR/${RELEASE_MUSL}/"
    ARCHIVE_MUSL="${RELEASE_MUSL}.tar.gz"
    tar -czf "target/${ARCHIVE_MUSL}" -C "$OUT_DIR" "${RELEASE_MUSL}"
    (cd target && sha512sum "${ARCHIVE_MUSL}" > "${ARCHIVE_MUSL}.sha512sum")
    echo -e "  ${GREEN}✓${NC} Archive: target/${ARCHIVE_MUSL}"
    echo -e "  ${GREEN}✓${NC} Checksum: target/${ARCHIVE_MUSL}.sha512sum"

    # Verify static linking
    echo ""
    echo "Verifying musl binary is static..."
    if ldd "$MUSL_BIN" 2>&1 | grep -q "not a dynamic executable"; then
        echo -e "  ${GREEN}✓${NC} Static binary (no dynamic dependencies)"
    else
        echo -e "  ${YELLOW}⚠${NC} Binary has dynamic dependencies:"
        ldd "$MUSL_BIN" 2>&1 | head -5
    fi
fi

# ── Verify checksums ──
echo ""
echo "Verifying checksums..."
(cd target && sha512sum -c "${ARCHIVE_GNU}.sha512sum")
echo -e "  ${GREEN}✓${NC} gnu checksum verified"
if [ -n "$MUSL_BIN" ]; then
    (cd target && sha512sum -c "${ARCHIVE_MUSL}.sha512sum")
    echo -e "  ${GREEN}✓${NC} musl checksum verified"
fi

echo ""
echo -e "${GREEN}━━━ release ready ━━━${NC}"
echo "  Archives:"
echo "    target/${ARCHIVE_GNU}"
echo "    target/${ARCHIVE_GNU}.sha512sum"
if [ -n "$MUSL_BIN" ]; then
    echo "    target/${ARCHIVE_MUSL}"
    echo "    target/${ARCHIVE_MUSL}.sha512sum"
fi
echo ""
echo "  Next: create GitHub Release and upload all files"
