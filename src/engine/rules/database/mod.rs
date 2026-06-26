// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Rule database — combined protected and cleanable rule sets.

mod cleanable;
mod protected;

use super::Rule;

pub(crate) fn build_rules() -> Vec<Rule> {
    let mut rules = protected::build_protected_rules();
    rules.extend(cleanable::build_cleanable_rules());
    rules
}
