// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Progress state machine for UX feedback.
//!
//! Gives users clear phase indication so they never wonder "is this running or hung?"

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

pub struct Progress {
    phase: Phase,
    started: Instant,
    spinner_idx: usize,
    quiet: bool,
}

impl Progress {
    pub fn new(quiet: bool) -> Self {
        Progress {
            phase: Phase::Scan,
            started: Instant::now(),
            spinner_idx: 0,
            quiet,
        }
    }

    pub fn advance(&mut self) {
        self.phase = self.phase.next();
        self.tick();
    }

    pub fn tick(&mut self) {
        if self.quiet {
            return;
        }
        let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let s = spinner[self.spinner_idx % spinner.len()];
        self.spinner_idx += 1;

        let elapsed = self.started.elapsed().as_secs_f64();
        print!(
            "\r  {} [{:5}] {:<10}  ({:.1}s)",
            s,
            self.phase.label(),
            phase_desc(self.phase),
            elapsed
        );
        io::stdout().flush().ok();
    }

    pub fn done(&self) {
        if self.quiet {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        println!(
            "\r  ✓ [{:5}] {:<10}  ({:.1}s)",
            self.phase.label(),
            phase_desc(self.phase),
            elapsed
        );
    }

    pub fn start_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.tick();
    }
}

fn phase_desc(phase: Phase) -> &'static str {
    match phase {
        Phase::Scan => "scanning...",
        Phase::Analyze => "analyzing...",
        Phase::Classify => "classifying...",
        Phase::Report => "reporting...",
        Phase::Ready => "done",
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
        assert_eq!(p.phase, Phase::Classify);
        p.advance();
        assert_eq!(p.phase, Phase::Report);
        p.advance();
        assert_eq!(p.phase, Phase::Ready);
    }

    #[test]
    fn test_phase_sequence() {
        let phases = vec![
            Phase::Scan,
            Phase::Analyze,
            Phase::Classify,
            Phase::Report,
            Phase::Ready,
        ];
        let labels: Vec<&str> = phases.iter().map(|p| p.label()).collect();
        assert_eq!(
            labels,
            vec!["SCAN", "ANALYZE", "CLASSIFY", "REPORT", "READY"]
        );
    }
}
