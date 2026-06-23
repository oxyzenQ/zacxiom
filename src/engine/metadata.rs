// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Filesystem metadata analysis — ELF detection, permissions, ownership.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// ELF magic bytes: 0x7F 'E' 'L' 'F'
const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

/// Detect whether a file is an ELF binary by reading magic bytes.
pub fn is_elf_binary(path: &Path) -> bool {
    if let Ok(data) = fs::read(path) {
        data.len() >= 4 && data[..4] == ELF_MAGIC
    } else {
        false
    }
}

/// Check if a file has the executable bit set.
pub fn is_executable(path: &Path) -> bool {
    if let Ok(meta) = fs::metadata(path) {
        let mode = meta.permissions().mode();
        mode & 0o111 != 0
    } else {
        false
    }
}

/// Get file size in bytes.
pub fn file_size(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.len())
}

/// Check if a path is a directory.
pub fn is_directory(path: &Path) -> bool {
    path.is_dir()
}

/// Check if a path is a regular file.
pub fn is_regular_file(path: &Path) -> bool {
    path.is_file()
}
