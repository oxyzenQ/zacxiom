// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Minimal color support — purple accent styling.
//!
//! v6.2.3: Purple spinner + section headers for visual identity.
//! Gracefully falls back to plain text when terminal doesn't support color.
//! Only colors: spinner, active status, section headers.
//! Never colors: body text, file paths, numbers, warnings.

use std::sync::atomic::{AtomicBool, Ordering};

static COLOR_OK: AtomicBool = AtomicBool::new(false);
static COLOR_INIT: AtomicBool = AtomicBool::new(false);

/// Initialize color detection. Call once at startup.
pub fn init() {
    if COLOR_INIT.swap(true, Ordering::Relaxed) {
        return;
    }
    let ok = detect_color();
    COLOR_OK.store(ok, Ordering::Relaxed);
}

fn detect_color() -> bool {
    // Check NO_COLOR first (https://no-color.org)
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    // Check TERM for dumb/unknown
    if let Ok(term) = std::env::var("TERM") {
        if term == "dumb" || term.is_empty() || term == "unknown" {
            return false;
        }
        if term.contains("256color") || term.contains("truecolor") {
            return true;
        }
    }
    // Check COLORTERM for truecolor support
    if let Ok(ct) = std::env::var("COLORTERM") {
        if ct == "truecolor" || ct == "24bit" {
            return true;
        }
    }
    // Check if stdout is a terminal
    #[cfg(target_os = "linux")]
    {
        let fd = libc::STDOUT_FILENO;
        if unsafe { libc::isatty(fd) } == 0 {
            return false;
        }
    }
    // Default: enable for common terminals
    true
}

pub fn is_enabled() -> bool {
    COLOR_OK.load(Ordering::Relaxed)
}

/// Purple RGB: #B478FF → (180, 120, 255)
const PURPLE_R: u8 = 180;
const PURPLE_G: u8 = 120;
const PURPLE_B: u8 = 255;

/// Wrap text in purple ANSI truecolor.
pub fn purple(text: &str) -> String {
    if !is_enabled() {
        return text.to_string();
    }
    format!(
        "\x1b[38;2;{r};{g};{b}m{text}\x1b[0m",
        r = PURPLE_R,
        g = PURPLE_G,
        b = PURPLE_B
    )
}

/// Purple spinner character.
pub fn purple_spinner(spin: char) -> String {
    if !is_enabled() {
        return spin.to_string();
    }
    format!(
        "\x1b[38;2;{r};{g};{b}m{spin}\x1b[0m",
        r = PURPLE_R,
        g = PURPLE_G,
        b = PURPLE_B
    )
}

/// Purple section header line.
pub fn section_header(name: &str) -> String {
    if !is_enabled() {
        return format!("\n{}\n{}\n", name, "─".repeat(name.len().min(60)));
    }
    format!(
        "\n\x1b[38;2;{r};{g};{b}m{name}\x1b[0m\n{sep}\n",
        r = PURPLE_R,
        g = PURPLE_G,
        b = PURPLE_B,
        sep = "─".repeat(name.len().min(60))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    /// Serialize env var access across parallel tests.
    /// std::env::set_var is NOT thread-safe on Linux (calls setenv).
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_no_color_respects_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let prev = env::var("NO_COLOR").ok();
        env::set_var("NO_COLOR", "1");
        let ok = detect_color();
        if let Some(v) = prev {
            env::set_var("NO_COLOR", &v);
        } else {
            env::remove_var("NO_COLOR");
        }
        assert!(!ok);
    }

    #[test]
    fn test_purple_no_color_plain() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // With no color, purple() returns plain text
        let prev = env::var("NO_COLOR").ok();
        env::set_var("NO_COLOR", "1");
        // Force re-init for test
        COLOR_OK.store(false, Ordering::Relaxed);
        COLOR_INIT.store(false, Ordering::Relaxed);
        init();
        let result = purple("hello");
        if let Some(v) = prev {
            env::set_var("NO_COLOR", &v);
        } else {
            env::remove_var("NO_COLOR");
        }
        // Reset for real use
        COLOR_INIT.store(false, Ordering::Relaxed);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_purple_spinner_format() {
        // Just verify it doesn't panic
        let _ = purple_spinner('⠋');
    }
}
