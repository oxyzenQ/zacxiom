// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Progress state machine v2 — live spinner + counters.
//!
//! v6.2.2: Added `set_status()` with live counters to prevent "frozen" perception.
//! v6.3:   Spinner animation frames for active-phase feedback.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
            Phase::Scan => "Discovering files",
            Phase::Analyze => "Building context",
            Phase::Classify => "Evaluating risk",
            Phase::Report => "Generating decisions",
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

const CLEAR: &str = "\r\x1b[K";
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct Progress {
    phase: Phase,
    started: Instant,
    quiet: bool,
    spinner_idx: usize,
    last_count: usize,
    running: Arc<AtomicBool>,
}

impl Progress {
    pub fn new(quiet: bool) -> Self {
        Progress {
            phase: Phase::Scan,
            started: Instant::now(),
            quiet,
            spinner_idx: 0,
            last_count: 0,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start a background spinner thread for long phases.
    pub fn start_spinner(&mut self) {
        if self.quiet {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let phase = self.phase;
        std::thread::spawn(move || {
            let mut idx = 0;
            while running.load(Ordering::SeqCst) {
                let spinner = SPINNER[idx % SPINNER.len()];
                print!(
                    "{}  {} [{:5}] {:<22}",
                    CLEAR,
                    spinner,
                    phase.label(),
                    phase.desc(),
                );
                io::stdout().flush().ok();
                idx += 1;
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
        });
    }

    /// Stop the spinner thread.
    pub fn stop_spinner(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Small delay so the spinner thread can exit
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    /// Advance to next phase, printing the completed phase line.
    pub fn advance(&mut self) {
        self.stop_spinner();
        if !self.quiet {
            let elapsed = self.started.elapsed().as_secs_f64();
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
        // Start spinner for new phase if it's long
        match self.phase {
            Phase::Scan | Phase::Analyze | Phase::Classify => {
                self.start_spinner();
            }
            _ => {}
        }
    }

    /// Mark complete — print final phase and newline.
    pub fn done(&mut self) {
        self.stop_spinner();
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
        println!();
    }

    /// Update status text with live counter (v6.2.2).
    pub fn status(&self, count: usize, total: usize, _label: &str) {
        if self.quiet {
            return;
        }
        let spinner = SPINNER[self.spinner_idx % SPINNER.len()];
        let pct = if total > 0 {
            (count as f64 / total as f64 * 100.0) as usize
        } else {
            0
        };
        let bar_width = 20;
        let filled = (pct * bar_width / 100).min(bar_width);
        print!(
            "{}  {} [{:5}] {:>7} / {:<7} files  {:<}",
            CLEAR,
            spinner,
            self.phase.label(),
            format_num(count),
            format_num(total),
            format!(
                "[{}{}] {}%",
                "█".repeat(filled),
                "░".repeat(bar_width - filled),
                pct
            ),
        );
        io::stdout().flush().ok();
    }
}

fn format_num(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
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
