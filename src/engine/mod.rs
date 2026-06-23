// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zacxiom Engine — reusable filesystem classification core.
//!
//! Architecture:
//!   classify(path) → ClassificationResult { category, risk, reasons, ... }
//!
//! Layers:
//!   1. Rule database — structured path patterns (highest priority)
//!   2. Metadata — ELF detection, permissions, ownership
//!   3. Regenerability — can this be safely recreated?
//!   4. Confidence scoring — combine evidence from all layers
//!
//! Independent from: explain, clean, simulate, reporting.
//! Consumable by: CLI, GUI, TUI, API, future tools.

pub mod classifier;
pub mod metadata;
pub mod rules;
pub mod types;

// Re-export the public API
pub use classifier::classify;
pub use types::{Category, ClassificationResult};
