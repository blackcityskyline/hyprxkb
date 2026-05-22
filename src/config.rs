//! Configuration: loading, defaults, writing the skeleton config, glob matching.

pub mod schema;
pub use schema::*;

use std::{fs, path::{Path, PathBuf}};

// ---------------------------------------------------------------------------
// Glob matching
// ---------------------------------------------------------------------------

/// Simple `*` / `?` glob match.
///
/// - `*` — matches any sequence of characters (including empty).
/// - `?` — matches exactly one character.
/// - Matching is case-insensitive (both sides are expected to be lowercased by caller).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (m, n) = (p.len(), t.len());
    let mut dp = vec![vec![false; n + 1]; m + 1];
    dp[0][0] = true;
    for i in 1..=m {
        if p[i - 1] == b'*' { dp[i][0] = dp[i - 1][0]; } else { break; }
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = match p[i - 1] {
                b'*' => dp[i - 1][j] || dp[i][j - 1],
                b'?' => dp[i - 1][j - 1],
                c    => dp[i - 1][j - 1] && c == t[j - 1],
            };
        }
    }
    dp[m][n]
}

// ---------------------------------------------------------------------------
// Rule matching
// ---------------------------------------------------------------------------

impl ForceLayoutConfig {
    /// Return the forced layout for a window class, or `None`.
    pub fn layout_for_class(&self, class: &str) -> Option<&str> {
        let lower = class.to_ascii_lowercase();
        self.rules.iter()
            .find(|r| r.apps.iter().any(|pat| glob_match(pat, &lower)))
            .map(|r| r.layout.as_str())
    }

    /// Return the forced layout for a layer surface name, or `None`.
    pub fn layout_for_layer(&self, layer: &str) -> Option<&str> {
        let lower = layer.to_ascii_lowercase();
        self.rules.iter()
            .find(|r| {
                r.layers.iter().any(|l| l.to_ascii_lowercase() == lower)
                    || r.layer_contains.iter().any(|sub| lower.contains(sub.as_str()))
            })
            .map(|r| r.layout.as_str())
    }
}

// ---------------------------------------------------------------------------
// Load / write
// ---------------------------------------------------------------------------

impl Config {
    pub fn load(path: &Path) -> Self {
        let text = match fs::read_to_string(path) {
            Ok(t)  => t,
            Err(e) => {
                eprintln!("[config] cannot read {path:?}: {e} — using defaults");
                return Self::default();
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(cfg) => {
                eprintln!("[config] loaded {path:?}");
                for w in cfg.warnings() {
                    eprintln!("[config] WARNING: {w}");
                }
                cfg
            }
            Err(e) => {
                eprintln!("[config] parse error in {path:?}: {e} — using defaults");
                Self::default()
            }
        }
    }

    /// Default config file path: `~/.config/hyprxkb/config.toml`.
    pub fn default_path() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(|home| {
            PathBuf::from(home).join(".config").join("hyprxkb").join("config.toml")
        })
    }

    /// Write the default config skeleton if the file does not yet exist.
    /// Returns `true` if a new file was created.
    pub fn write_default(path: &Path) -> bool {
        if path.exists() {
            return false;
        }
        if let Some(dir) = path.parent() {
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!("[config] cannot create {dir:?}: {e}");
                return false;
            }
        }
        match fs::write(path, DEFAULT_CONFIG_TEMPLATE) {
            Ok(()) => {
                eprintln!("[config] created default config at {path:?}");
                true
            }
            Err(e) => {
                eprintln!("[config] cannot write {path:?}: {e}");
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Default config template
// ---------------------------------------------------------------------------

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# hyprxkb — keyboard layout switcher for Hyprland
# ~/.config/hyprxkb/config.toml
#
# Run `hyprxkb init` to (re)create this file.
# Run `hyprctl devices` to find your keyboard device name.

# ── Keyboard ──────────────────────────────────────────────────────────────────
[keyboard]
# Device name as shown by `hyprctl devices` (case-sensitive).
# Use "all" to switch every keyboard Hyprland knows about simultaneously —
# useful when you have keyd or other virtual keyboards alongside the physical one.
device  = "all"

# Layout rotation order — use XKB identifiers (us, ru, de, gb, fr, …).
# `hyprxkb switch` and hotkey cycle through these in order.
layouts = ["us", "ru"]

# ── Hotkey ────────────────────────────────────────────────────────────────────
[hotkey]
# Modifier: Super | Alt | Ctrl | Shift
modifier = "Super"
# Key:      Space | Tab | Grave | Minus | Equal | F1–F12 | Left | Right | …
key      = "Space"

# ── Labels ────────────────────────────────────────────────────────────────────
# Human-readable names shown in notifications and `hyprxkb status`.
# Keys must be XKB layout identifiers matching keyboard.layouts above.
[labels]
us = "🇺🇸 English"
ru = "🇷🇺 Russian"

# ── Notifications ─────────────────────────────────────────────────────────────
[notify]
# backend: none | swayosd | notify-send | quickshell
backend    = "none"
timeout_ms = 2000
icon       = "input-keyboard-symbolic"

# Instantly refresh waybar after every layout switch.
# Set the signal number matching your waybar module's "signal" field.
# waybar_signal = 8

# QuickShell IPC socket (only used when backend = "quickshell").
# Defaults to $XDG_RUNTIME_DIR/quickshell.sock
# quickshell_socket = "/run/user/1000/quickshell.sock"

# ── CapsLock ──────────────────────────────────────────────────────────────────
[capslock]
# Monitor /sys/class/leds/ and send a notification on CapsLock state change.
enabled = true
poll_ms = 150

# ── General ───────────────────────────────────────────────────────────────────
[general]
# File used to persist the active layout across restarts.
state_file = "/tmp/hyprxkb-state"

# Minimum delay (ms) between automatic switches triggered by window focus.
# Prevents flickering when focus changes rapidly. Does not affect hotkey.
switch_delay_ms = 100

# Remember the last layout per app and restore it on focus (like Windows IME).
per_window_memory = false

# How often (ms) to re-read the real compositor layout and sync our state.
# Keeps us correct when another tool changes the layout externally.
sync_interval_ms = 5000

# ── Force-layout rules ────────────────────────────────────────────────────────
# Force a specific layout when matching windows or layer surfaces are active.
# Rules are evaluated top-to-bottom; the first match wins.
# App patterns support * (any sequence) and ? (any single char) wildcards.
# Matching is case-insensitive.

[[force_layout.rules]]
layout = "us"
# Terminal emulators and TUI apps — always English
apps = [
    "org.alacritty", "foot", "kitty", "org.wezfurlong.wezterm",
    "nvim", "vim", "btop", "htop", "mpv", "pcmanfm",
]
# Launcher layer surfaces
layers         = ["rofi", "wofi"]
layer_contains = ["launcher", "runner"]

# Example: force Russian for messaging apps
# [[force_layout.rules]]
# layout = "ru"
# apps   = ["org.telegram.desktop", "discord", "org.telegram.*"]
"#;
