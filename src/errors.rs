// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Error classification layer — no raw OS errors in user output.
//!
//! Classifies low-level errors into user-facing categories.

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ErrorKind {
    InUse,
    PermissionDenied,
    SystemFile,
    LockedProcess,
    NotFound,
    Unknown,
}

impl ErrorKind {
    pub fn from_io_error(err: &std::io::Error) -> Self {
        use std::io::ErrorKind as Io;
        match err.kind() {
            Io::PermissionDenied => ErrorKind::PermissionDenied,
            Io::NotFound => ErrorKind::NotFound,
            _ => {
                let msg = err.to_string().to_lowercase();
                if msg.contains("permission") {
                    ErrorKind::PermissionDenied
                } else if msg.contains("text file busy") || msg.contains("in use") {
                    ErrorKind::InUse
                } else {
                    ErrorKind::Unknown
                }
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ErrorKind::InUse => "IN_USE",
            ErrorKind::PermissionDenied => "PERMISSION_DENIED",
            ErrorKind::SystemFile => "SYSTEM_FILE",
            ErrorKind::LockedProcess => "LOCKED_PROCESS",
            ErrorKind::NotFound => "NOT_FOUND",
            ErrorKind::Unknown => "UNKNOWN",
        }
    }

    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            ErrorKind::InUse => "File in use by a running process",
            ErrorKind::PermissionDenied => "Insufficient permissions to access",
            ErrorKind::SystemFile => "Protected system file — not removable",
            ErrorKind::LockedProcess => "Process holds an exclusive lock",
            ErrorKind::NotFound => "File not found (deleted or moved)",
            ErrorKind::Unknown => "Unknown error condition",
        }
    }
}

/// Aggregated error statistics for summary output.
#[derive(Default)]
pub struct ErrorSummary {
    pub permission_denied: usize,
    pub system_protected: usize,
    pub in_use: usize,
    pub locked: usize,
    pub not_found: usize,
    pub unknown: usize,
}

impl ErrorSummary {
    pub fn total(&self) -> usize {
        self.permission_denied
            + self.system_protected
            + self.in_use
            + self.locked
            + self.not_found
            + self.unknown
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        if self.permission_denied > 0 {
            out.push_str(&format!(
                "  Skipped: {} files (permission denied)\n",
                self.permission_denied
            ));
        }
        if self.system_protected > 0 {
            out.push_str(&format!(
                "  Skipped: {} system protected paths\n",
                self.system_protected
            ));
        }
        if self.in_use > 0 {
            out.push_str(&format!(
                "  Skipped: {} files (in use by processes)\n",
                self.in_use
            ));
        }
        if self.locked > 0 {
            out.push_str(&format!("  Skipped: {} locked files\n", self.locked));
        }
        if self.not_found > 0 {
            out.push_str(&format!(
                "  Skipped: {} files (not found)\n",
                self.not_found
            ));
        }
        if self.unknown > 0 {
            out.push_str(&format!("  Errors: {} unknown errors\n", self.unknown));
        }
        out
    }
}
