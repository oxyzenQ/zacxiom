// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Progress state machine — clean, no-flicker phase transitions.
//!
//! Each phase prints exactly one line and advances.
//! Carriage return + clear ensures old text is fully overwritten.

use std::io::{self, Write};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Scan,
    Analyze,
    Classify,
    Report,
    Ready,
}

impl Phase {
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Scan => "SCAN",
            Phase::Analyze => "ANALYZE",
            Phase::Classify => "CLASSIFY",
            Phase::Report => "REPORT",
            Phase::Ready => "READY",
        }
    }

    fn desc(&self) -> &'static str {
        match self {
            Phase::Scan => "Discovering files...",
            Phase::Analyze => "Building context...",
            Phase::Classify => "Evaluating risk...",
            Phase::Report => "Generating decisions...",
            Phase::Ready => "Complete",
        }
    }

    pub fn next(&self) -> Phase {
        match self {
            Phase::Scan => Phase::Analyze,
            Phase::Analyze => Phase::Classify,
            Phase::Classify => Phase::Report,
            Phase::Report => Phase::Ready,
            Phase::Ready => Phase::Ready,
        }
    }
}

const CLEAR: &str = "\r\x1b[K"; // carriage return + clear to end of line

pub struct Progress {
    phase: Phase,
    started: Instant,
    quiet: bool,
}

impl Progress {
    pub fn new(quiet: bool) -> Self {
        Progress {
            phase: Phase::Scan,
            started: Instant::now(),
            quiet,
        }
    }

    /// Advance to next phase, printing the completed phase line.
    pub fn advance(&mut self) {
        if !self.quiet {
            let elapsed = self.started.elapsed().as_secs_f64();
            // Clear current line, print completed phase
            println!(
                "{}  ✓ [{:5}] {:<22} ({:.1}s)",
                CLEAR,
                self.phase.label(),
                self.phase.desc(),
                elapsed
            );
            io::stdout().flush().ok();
        }
        self.phase = self.phase.next();
        if !self.quiet {
            print!(
                "{}  ⠋ [{:5}] {:<22}",
                CLEAR,
                self.phase.label(),
                self.phase.desc(),
            );
            io::stdout().flush().ok();
        }
    }

    /// Mark complete — print final phase and newline.
    pub fn done(&mut self) {
        if self.quiet {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        self.phase = Phase::Ready;
        println!(
            "{}  ✓ [{:5}] {:<22} ({:.1}s)",
            CLEAR,
            self.phase.label(),
            self.phase.desc(),
            elapsed
        );
        println!(); // blank line before output begins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_advances() {
        let mut p = Progress::new(true);
        assert_eq!(p.phase, Phase::Scan);
        p.advance();
        assert_eq!(p.phase, Phase::Analyze);
        p.advance();
        p.advance();
        assert_eq!(p.phase, Phase::Report);
    }

    #[test]
    fn test_phase_sequence() {
        let phases = [
            Phase::Scan,
            Phase::Analyze,
            Phase::Classify,
            Phase::Report,
            Phase::Ready,
        ];
        let labels: Vec<&str> = phases.iter().map(|p| p.label()).collect();
        assert_eq!(labels, ["SCAN", "ANALYZE", "CLASSIFY", "REPORT", "READY"]);
    }
}
