// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI command implementations.
//!
//! Each subcommand lives in its own file. The dispatch table is in main.rs.

pub mod check_update;
pub mod clean;
pub mod explain;
pub mod explain_confidence;
pub mod explain_risk;
pub mod inspect;
pub mod plan;
pub mod report;
pub mod scan;
pub mod status;
pub mod undo;

pub use check_update::check_update;
pub use clean::run_clean;
pub use explain::run_explain;
pub use explain_confidence::run_explain_confidence;
pub use explain_risk::run_explain_risk;
pub use inspect::run_inspect_unknown;
pub use plan::run_plan;
pub use report::run_simulate;
pub use scan::run_scan;
pub use status::run_status;
pub use undo::run_undo;
