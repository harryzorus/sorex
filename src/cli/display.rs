// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Terminal display utilities for sorex CLI.
//!
//! Pretty terminal output that respects your color scheme. OneDark for dark
//! terminals, One Light for light ones. The detection tries `SOREX_THEME` first
//! (for explicit control), then `COLORFGBG` (set by some terminals), then macOS
//! system appearance, then defaults to dark because most developers live there.
//!
//! Box drawing, tier badges, savings percentages, timing colors - all the little
//! touches that make CLI output feel polished. Respects `NO_COLOR` for the purists
//! and non-TTY detection for pipelines.
//!
//! # Theme detection order
//!
//! 1. `SOREX_THEME` env var ("dark" or "light")
//! 2. `COLORFGBG` env var (terminal background hint)
//! 3. macOS appearance (via defaults read)
//! 4. Default to dark theme

use std::sync::OnceLock;

// Box drawing constants - width between │ and │ (excluding border chars)
pub const BOX_WIDTH: usize = 80;

// ═══════════════════════════════════════════════════════════════════════════
// THEME DETECTION
// ═══════════════════════════════════════════════════════════════════════════

/// Terminal color theme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

/// Cached theme detection result
static THEME: OnceLock<Theme> = OnceLock::new();

/// Detect terminal theme from environment
fn detect_theme() -> Theme {
    // 1. Explicit override via SOREX_THEME
    if let Ok(theme) = std::env::var("SOREX_THEME") {
        match theme.to_lowercase().as_str() {
            "light" | "l" => return Theme::Light,
            "dark" | "d" => return Theme::Dark,
            _ => {}
        }
    }

    // 2. COLORFGBG (format: "fg;bg" where bg > 6 typically means light)
    // Set by some terminals like xterm, rxvt
    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        if let Some(bg) = colorfgbg.split(';').next_back() {
            if let Ok(bg_num) = bg.parse::<u8>() {
                // Colors 0-6 are typically dark, 7+ are light
                // 15 = white, 0 = black
                if bg_num >= 7 && bg_num != 8 {
                    return Theme::Light;
                }
            }
        }
    }

    // 3. macOS: Check system appearance
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            // "Dark" means dark mode; absence or error means light mode
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("Dark") && output.status.success() {
                return Theme::Light;
            }
        }
    }

    // 4. Default to dark (most developer terminals)
    Theme::Dark
}

/// Get the current theme (cached)
pub fn theme() -> Theme {
    *THEME.get_or_init(detect_theme)
}

// ═══════════════════════════════════════════════════════════════════════════
// ONEDARK / ONE LIGHT COLOR PALETTES (True Color)
// ═══════════════════════════════════════════════════════════════════════════
//
// OneDark: https://github.com/joshdick/onedark.vim
// One Light: https://github.com/sonph/onehalf

/// True color escape sequence helper
fn rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
}

pub use colors::*;

/// OneDark palette
mod onedark {
    pub const RED: (u8, u8, u8) = (224, 108, 117);     // #e06c75
    pub const GREEN: (u8, u8, u8) = (152, 195, 121);   // #98c379
    pub const YELLOW: (u8, u8, u8) = (229, 192, 123);  // #e5c07b
    pub const BLUE: (u8, u8, u8) = (97, 175, 239);     // #61afef
    pub const MAGENTA: (u8, u8, u8) = (198, 120, 221); // #c678dd
    pub const CYAN: (u8, u8, u8) = (86, 182, 194);     // #56b6c2
    pub const WHITE: (u8, u8, u8) = (171, 178, 191);   // #abb2bf
    pub const GRAY: (u8, u8, u8) = (92, 99, 112);      // #5c6370
    pub const BRIGHT_RED: (u8, u8, u8) = (240, 113, 120);
    pub const BRIGHT_GREEN: (u8, u8, u8) = (166, 226, 46);
    pub const BRIGHT_YELLOW: (u8, u8, u8) = (255, 215, 0);
    pub const BRIGHT_BLUE: (u8, u8, u8) = (127, 200, 255);
    pub const BRIGHT_MAGENTA: (u8, u8, u8) = (224, 145, 237); // #e091ed
    pub const BRIGHT_CYAN: (u8, u8, u8) = (102, 217, 239);
}

/// One Light palette
mod onelight {
    pub const RED: (u8, u8, u8) = (228, 86, 73);       // #e45649
    pub const GREEN: (u8, u8, u8) = (80, 161, 79);     // #50a14f
    pub const YELLOW: (u8, u8, u8) = (193, 132, 1);    // #c18401
    pub const BLUE: (u8, u8, u8) = (64, 120, 242);     // #4078f2
    pub const MAGENTA: (u8, u8, u8) = (166, 38, 164);  // #a626a4
    pub const CYAN: (u8, u8, u8) = (1, 132, 188);      // #0184bc
    pub const WHITE: (u8, u8, u8) = (56, 58, 66);      // #383a42 (foreground)
    pub const GRAY: (u8, u8, u8) = (160, 161, 167);    // #a0a1a7
    pub const BRIGHT_RED: (u8, u8, u8) = (202, 18, 67);
    pub const BRIGHT_GREEN: (u8, u8, u8) = (68, 140, 39);
    pub const BRIGHT_YELLOW: (u8, u8, u8) = (152, 104, 1);
    pub const BRIGHT_BLUE: (u8, u8, u8) = (54, 100, 212);
    pub const BRIGHT_MAGENTA: (u8, u8, u8) = (146, 38, 144); // #922690
    pub const BRIGHT_CYAN: (u8, u8, u8) = (1, 112, 158);
}

// ═══════════════════════════════════════════════════════════════════════════
// THEME-AWARE COLOR ACCESSORS
// ═══════════════════════════════════════════════════════════════════════════

macro_rules! theme_color {
    ($name:ident) => {
        #[allow(non_snake_case)]
        pub fn $name() -> String {
            let (r, g, b) = match theme() {
                Theme::Dark => onedark::$name,
                Theme::Light => onelight::$name,
            };
            rgb(r, g, b)
        }
    };
}

theme_color!(RED);
theme_color!(GREEN);
theme_color!(YELLOW);
theme_color!(BLUE);
theme_color!(MAGENTA);
theme_color!(CYAN);
theme_color!(WHITE);
theme_color!(GRAY);
theme_color!(BRIGHT_RED);
theme_color!(BRIGHT_GREEN);
theme_color!(BRIGHT_YELLOW);
theme_color!(BRIGHT_BLUE);
theme_color!(BRIGHT_MAGENTA);
theme_color!(BRIGHT_CYAN);

// ═══════════════════════════════════════════════════════════════════════════
// CORE UTILITIES
// ═══════════════════════════════════════════════════════════════════════════

/// Check if colors should be used (TTY detection)
pub fn use_colors() -> bool {
    // Respect NO_COLOR standard
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    atty::is(atty::Stream::Stdout)
}

/// Apply color if TTY, otherwise return plain text
#[allow(dead_code)]
pub fn color(c: &str, text: &str) -> String {
    if use_colors() {
        format!("{}{}{}", c, text, RESET)
    } else {
        text.to_string()
    }
}

/// Apply multiple styles
pub fn styled(styles: &[&str], text: &str) -> String {
    if use_colors() {
        format!("{}{}{}", styles.join(""), text, RESET)
    } else {
        text.to_string()
    }
}

/// Apply theme color with optional modifiers
pub fn themed(color_fn: fn() -> String, modifiers: &[&str], text: &str) -> String {
    if use_colors() {
        format!("{}{}{}{}", modifiers.join(""), color_fn(), text, RESET)
    } else {
        text.to_string()
    }
}

/// Calculate visible length (excluding ANSI codes)
pub fn visible_len(s: &str) -> usize {
    let mut in_escape = false;
    let mut len = 0;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape && c == 'm' {
            in_escape = false;
        } else if !in_escape {
            len += 1;
        }
    }
    len
}

// ═══════════════════════════════════════════════════════════════════════════
// BOX DRAWING
// ═══════════════════════════════════════════════════════════════════════════

/// Print a content line: │ content          │
pub fn row(content: &str) {
    let border = GRAY();
    let len = visible_len(content);
    let pad = BOX_WIDTH.saturating_sub(len);
    println!(
        "{}│{}{}{}{}│{}",
        border,
        RESET,
        content,
        " ".repeat(pad),
        border,
        RESET
    );
}

/// Print section header: ┌─ LABEL ──────────┐
pub fn section_top(label: &str) {
    let border = GRAY();
    let colored_label = themed(CYAN, &[BOLD], label);
    let label_part = format!("─ {} ", colored_label);
    let remaining = BOX_WIDTH - visible_len(&label_part);
    println!(
        "{}┌{}{}{}{}┐{}",
        border,
        RESET,
        label_part,
        border,
        "─".repeat(remaining),
        RESET
    );
}

/// Print section divider: ├─ LABEL ──────────┤
pub fn section_mid(label: &str) {
    let border = GRAY();
    let colored_label = themed(CYAN, &[BOLD], label);
    let label_part = format!("─ {} ", colored_label);
    let remaining = BOX_WIDTH - visible_len(&label_part);
    println!(
        "{}├{}{}{}─{}┤{}",
        border,
        RESET,
        label_part,
        border,
        "─".repeat(remaining - 1),
        RESET
    );
}

/// Print section footer: └──────────────────┘
pub fn section_bot() {
    let border = GRAY();
    println!("{}└{}┘{}", border, "─".repeat(BOX_WIDTH), RESET);
}

/// Print double-line header: ╔══════════════════╗
pub fn double_header() {
    let border = BLUE();
    println!("{}╔{}╗{}", border, "═".repeat(BOX_WIDTH), RESET);
}

/// Print double-line divider: ╠══════════════════╣
pub fn double_divider() {
    let border = BLUE();
    println!("{}╠{}╣{}", border, "═".repeat(BOX_WIDTH), RESET);
}

/// Print double-line footer: ╚══════════════════╝
pub fn double_footer() {
    let border = BLUE();
    println!("{}╚{}╝{}", border, "═".repeat(BOX_WIDTH), RESET);
}

/// Print centered content line: ║      TEXT        ║
pub fn row_double(content: &str) {
    let border = BLUE();
    let len = visible_len(content);
    let pad = BOX_WIDTH.saturating_sub(len);
    println!(
        "{}║{}{}{}{}║{}",
        border,
        RESET,
        content,
        " ".repeat(pad),
        border,
        RESET
    );
}

/// Print centered title with bold
pub fn title(text: &str) {
    let border = BLUE();
    let colored = themed(BRIGHT_CYAN, &[BOLD], text);
    let len = visible_len(&colored);
    let total_pad = BOX_WIDTH.saturating_sub(len);
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    println!(
        "{}║{}{}{}{}{}║{}",
        border,
        RESET,
        " ".repeat(left_pad),
        colored,
        " ".repeat(right_pad),
        border,
        RESET
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// SEMANTIC FORMATTERS
// ═══════════════════════════════════════════════════════════════════════════

/// Color-coded encoding technique badge
pub fn technique_badge(tech: &str) -> String {
    if !use_colors() {
        return format!("[{}]", tech);
    }
    let color = match tech {
        "FC" => GREEN(),
        "STRM" => BLUE(),
        "DELTA" => MAGENTA(),
        "SKIP" => YELLOW(),
        "DEDUP" => CYAN(),
        "DFA" => BRIGHT_MAGENTA(),
        "BIN" => BRIGHT_BLUE(),
        "RAW" => GRAY(),
        "DICT" => BRIGHT_GREEN(),
        "CRC" => BRIGHT_RED(),
        _ => return format!("[{}]", tech),
    };
    format!("{}[{}]{}", color, tech, RESET)
}

/// Format savings percentage with color
pub fn savings_colored(raw: usize, compressed: usize) -> String {
    if raw == 0 {
        return themed(GRAY, &[], "   N/A");
    }
    let saved_pct = (1.0 - compressed as f64 / raw as f64) * 100.0;
    if saved_pct.abs() < 0.5 {
        themed(GRAY, &[], "    0%")
    } else if saved_pct > 0.0 {
        themed(GREEN, &[BOLD], &format!("{:>5.0}%", saved_pct))
    } else {
        themed(RED, &[BOLD], &format!("{:>+5.0}%", saved_pct))
    }
}

/// Left-pad a styled string to a fixed visible width
pub fn pad_left(s: &str, width: usize) -> String {
    let visible = visible_len(s);
    if visible >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - visible), s)
    }
}

/// Right-pad a styled string to a fixed visible width
pub fn pad_right(s: &str, width: usize) -> String {
    let visible = visible_len(s);
    if visible >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - visible))
    }
}

/// Format bytes as human-readable size
pub fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a path to max_len, adding ... prefix if needed
pub fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - max_len + 3..])
    }
}

/// Color-coded tier label
pub fn tier_label(tier: u8) -> String {
    if !use_colors() {
        return format!("T{}", tier);
    }
    match tier {
        1 => format!("{}T1{}", GREEN(), RESET),
        2 => format!("{}T2{}", BLUE(), RESET),
        3 => format!("{}T3{}", MAGENTA(), RESET),
        _ => format!("T{}", tier),
    }
}

/// Color-coded match type label
pub fn match_type_label(match_type: &str) -> String {
    if !use_colors() {
        return match_type.to_string();
    }
    match match_type {
        "Title" => format!("{}{}{}", BRIGHT_GREEN(), match_type, RESET),
        "Section" => format!("{}{}{}", CYAN(), match_type, RESET),
        "Subheading" => format!("{}{}{}", BLUE(), match_type, RESET),
        "Subheading2" => format!("{}{}{}", BLUE(), match_type, RESET),
        "Content" => format!("{}{}{}", GRAY(), match_type, RESET),
        _ => match_type.to_string(),
    }
}

/// Color-coded timing value (green=fast, yellow=medium, red=slow)
pub fn timing_us(value: f64) -> String {
    if !use_colors() {
        return format!("{:>10.3}", value);
    }
    let color = if value < 10.0 {
        GREEN()
    } else if value < 100.0 {
        YELLOW()
    } else if value < 1000.0 {
        BRIGHT_YELLOW()
    } else {
        RED()
    };
    format!("{}{:>10.3}{}", color, value, RESET)
}

/// Color-coded timing value in ms
pub fn timing_ms(value: f64) -> String {
    if !use_colors() {
        return format!("{:>10.3}", value);
    }
    let color = if value < 5.0 {
        GREEN()
    } else if value < 20.0 {
        YELLOW()
    } else {
        RED()
    };
    format!("{}{:>10.3}{}", color, value, RESET)
}

/// Color-coded score value
pub fn score_value(score: f64) -> String {
    if !use_colors() {
        return format!("{:>7.0}", score);
    }
    let color = if score >= 100.0 {
        BRIGHT_GREEN()
    } else if score >= 50.0 {
        GREEN()
    } else if score >= 20.0 {
        YELLOW()
    } else {
        GRAY()
    };
    format!("{}{:>7.0}{}", color, score, RESET)
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_len_no_escapes() {
        assert_eq!(visible_len("hello"), 5);
        assert_eq!(visible_len(""), 0);
    }

    #[test]
    fn test_visible_len_with_escapes() {
        let colored = "\x1b[32mhello\x1b[0m".to_string();
        assert_eq!(visible_len(&colored), 5);
    }

    #[test]
    fn test_rgb_format() {
        let code = rgb(255, 128, 64);
        assert_eq!(code, "\x1b[38;2;255;128;64m");
    }

    #[test]
    fn test_theme_colors_are_different() {
        // OneDark and OneLight should have different RGB values
        assert_ne!(onedark::RED, onelight::RED);
        assert_ne!(onedark::GREEN, onelight::GREEN);
        assert_ne!(onedark::BLUE, onelight::BLUE);
    }
}
