#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# golden-update.sh — Regenerate golden test files after intentional CLI changes.
# Usage: ./scripts/golden-update.sh
set -euo pipefail

cd "$(dirname "$0")/.."

GOLDEN_DIR="tests/golden"
ZACXIOM=target/release/zacxiom

echo "Regenerating golden files..."

mkdir -p "$GOLDEN_DIR"
"$ZACXIOM" help           > "$GOLDEN_DIR/help.txt"   2>&1
"$ZACXIOM" status --golden > "$GOLDEN_DIR/status.txt" 2>&1
"$ZACXIOM" doctor --golden > "$GOLDEN_DIR/doctor.txt" 2>&1

echo "Done. Updated:"
ls -la "$GOLDEN_DIR/"
