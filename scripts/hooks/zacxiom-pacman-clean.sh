#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# pacman post-transaction hook for zacxiom (v13.3)
#
# Cleans package manager cache after pacman transactions.
# This is OPT-IN — users must manually install this hook.
#
# Installation:
#   sudo cp scripts/hooks/zacxiom-pacman.hook /usr/share/libalpm/hooks/
#   sudo cp scripts/hooks/zacxiom-pacman-clean.sh /usr/local/bin/
#   sudo chmod +x /usr/local/bin/zacxiom-pacman-clean.sh
#
# Removal:
#   sudo rm /usr/share/libalpm/hooks/zacxiom-pacman.hook
#   sudo rm /usr/local/bin/zacxiom-pacman-clean.sh
#
# The hook runs zacxiom in safe mode (--smart --yes) to clean:
#   - /var/cache/pacman/pkg (package cache)
#   - ~/.cache (user caches)
#
# All deletions are recoverable via 'zacxiom undo'.

set -euo pipefail

# Only run if zacxiom is installed
if ! command -v zacxiom &>/dev/null; then
    exit 0
fi

# Run zacxiom in smart mode — safe + low-risk files only
# --yes skips confirmation (non-interactive)
# --quiet suppresses progress output (hook output goes to pacman log)
# --fail-fast stops on first error (don't block pacman)
zacxiom clean --smart --yes --quiet --fail-fast 2>&1 || true

exit 0
