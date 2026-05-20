//! Configuration: loading, defaults, writing the skeleton config file.

use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

// ---------------------------------------------------------------------------
// Notify
// ---------------------------------------------------------------------------

/// Supported notification backends.
///
/// `Dunst`, `Mako`, `SwayNc`, and `NotifySend` all call `notify-send` under
/// the hood — they exist as separate variants only for documentation purposes
/// and possible future divergence.
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum NotifyBackend {
    /// Notifications disabled.
    #[default]
    None,
    /// `swayosd-client` — shows OSD overlays.
    SwayOsd,
    /// `notify-send` compatible daemons (dunst, mako, swaync, libnotify).
    NotifySend,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotifyConfig {
    pub backend:    NotifyBackend,
    pub timeout_ms: u64,
    pub icon:       String,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            backend:    NotifyBackend::None,
            timeout_ms: 2000,
            icon:       "input-keyboard-symbolic".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// CapsLock
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CapsLockConfig {
    pub enabled: bool,
    pub poll_ms: u64,
}

impl Default for CapsLockConfig {
    fn default() -> Self {
        Self { enabled: true, poll_ms: 150 }
    }
}

// ---------------------------------------------------------------------------
// Keyboard
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeyboardConfig {
    /// The hyprctl device name (see `hyprctl devices`).
    pub device:  String,
    /// Layout rotation order — names must match XKB layout names.
    pub layouts: Vec<String>,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            device:  "keyd-virtual-keyboard".into(),
            layouts: vec!["en".into(), "ru".into()],
        }
    }
}

// ---------------------------------------------------------------------------
// Hotkey
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    /// `Meta` | `Alt` | `Ctrl` | `Shift`
    pub modifier: String,
    /// `Space` | `Tab` | `F1`–`F12` | `Grave` | `Minus` | …
    pub key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self { modifier: "Meta".into(), key: "Space".into() }
    }
}

// ---------------------------------------------------------------------------
// Force-layout rules
// ---------------------------------------------------------------------------

/// A single rule that forces a layout for specific apps or layer surfaces.
#[derive(Debug, Clone, Deserialize)]
pub struct ForceRule {
    /// Target layout name (must be in `keyboard.layouts`).
    pub layout: String,
    /// Exact window class names (lowercased) that trigger this rule.
    #[serde(default)]
    pub apps: Vec<String>,
    /// Exact layer surface names that trigger this rule.
    #[serde(default)]
    pub layers: Vec<String>,
    /// Substring match for layer surface names.
    #[serde(default)]
    pub layer_contains: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ForceLayoutConfig {
    pub rules: Vec<ForceRule>,
}

impl Default for ForceLayoutConfig {
    fn default() -> Self {
        Self {
            rules: vec![ForceRule {
                layout: "en".into(),
                apps: vec![
                    "nvim".into(), "vim".into(), "btop".into(), "htop".into(),
                    "alacritty".into(), "foot".into(), "kitty".into(),
                    "mpv".into(), "pcmanfm".into(),
                ],
                layers:         vec!["rofi".into(), "wofi".into()],
                layer_contains: "launcher".into(),
            }],
        }
    }
}

impl ForceLayoutConfig {
    pub fn layout_for_class(&self, class: &str) -> Option<&str> {
        self.rules.iter()
            .find(|r| r.apps.iter().any(|a| a == class))
            .map(|r| r.layout.as_str())
    }

    pub fn layout_for_layer(&self, layer: &str) -> Option<&str> {
        self.rules.iter()
            .find(|r| {
                r.layers.iter().any(|l| l == layer)
                    || (!r.layer_contains.is_empty() && layer.contains(&r.layer_contains))
            })
            .map(|r| r.layout.as_str())
    }
}

// ---------------------------------------------------------------------------
// General
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// File used to persist the current layout across restarts.
    pub layout_file:     String,
    /// Minimum time between layout switches (debounce for Hyprland events).
    pub switch_delay_ms: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { layout_file: "/tmp/hypr-layout".into(), switch_delay_ms: 150 }
    }
}

// ---------------------------------------------------------------------------
// Root config
// ---------------------------------------------------------------------------

/// Human-readable display names for each layout (used in notifications).
/// Key: layout name (e.g. "en"), value: display string (e.g. "🇺🇸 English").
pub type LayoutMessages = HashMap<String, String>;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub keyboard:     KeyboardConfig,
    pub hotkey:       HotkeyConfig,
    pub notify:       NotifyConfig,
    pub capslock:     CapsLockConfig,
    pub force_layout: ForceLayoutConfig,
    pub general:      GeneralConfig,
    pub messages:     LayoutMessages,
}

impl Config {
    /// Load config from `path`, falling back to defaults on any error.
    pub fn load(path: &Path) -> Self {
        let text = match fs::read_to_string(path) {
            Ok(t)  => t,
            Err(e) => {
                eprintln!("[config] cannot read {path:?}: {e} — using defaults");
                return Self::default();
            }
        };
        match toml::from_str(&text) {
            Ok(cfg) => { eprintln!("[config] loaded: {path:?}"); cfg }
            Err(e)  => {
                eprintln!("[config] parse error in {path:?}: {e} — using defaults");
                Self::default()
            }
        }
    }

    /// Display name for `layout` (falls back to the layout name itself).
    pub fn layout_message<'a>(&'a self, layout: &'a str) -> &'a str {
        self.messages.get(layout).map(String::as_str).unwrap_or(layout)
    }

    /// Index of `layout` in `keyboard.layouts`, used by `hyprctl switchxkblayout`.
    pub fn layout_index(&self, layout: &str) -> Option<usize> {
        self.keyboard.layouts.iter().position(|l| l == layout)
    }

    pub fn layout_for_class(&self, class: &str) -> Option<&str> {
        self.force_layout.layout_for_class(class)
    }

    pub fn layout_for_layer(&self, layer: &str) -> Option<&str> {
        self.force_layout.layout_for_layer(layer)
    }

    pub fn default_path() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(|home| {
            PathBuf::from(home).join(".config").join("hyprxkb").join("config.toml")
        })
    }

    /// Write the default config skeleton if the file does not yet exist.
    pub fn write_default(path: &Path) {
        if path.exists() { return; }
        if let Some(dir) = path.parent() {
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!("[config] mkdir {dir:?}: {e}");
                return;
            }
        }
        if let Err(e) = fs::write(path, DEFAULT_CONFIG) {
            eprintln!("[config] write {path:?}: {e}");
        } else {
            eprintln!("[config] created default: {path:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Default config file template
// ---------------------------------------------------------------------------

const DEFAULT_CONFIG: &str = r#"# hyprxkb configuration — ~/.config/hyprxkb/config.toml

[keyboard]
device  = "keyd-virtual-keyboard"   # see: hyprctl devices
layouts = ["en", "ru"]              # rotation order

[hotkey]
modifier = "Meta"   # Meta | Alt | Ctrl | Shift
key      = "Space"  # Space | Tab | F1..F12 | Grave | Minus | ...

# Human-readable names shown in notifications.
[messages]
en = "🇺🇸 US English"
ru = "🇷🇺 Russian"

# Notification backend.
# backend: none | swayosd | notify-send
[notify]
backend    = "none"
timeout_ms = 2000
icon       = "input-keyboard-symbolic"

# CapsLock state monitoring (reads /sys/class/leds/).
[capslock]
enabled = true
poll_ms = 150

# Force a specific layout when certain apps or layer surfaces are active.
# Multiple rules are evaluated in order; first match wins.
[[force_layout.rules]]
layout         = "en"
apps           = ["nvim", "vim", "btop", "htop", "alacritty", "foot", "kitty", "mpv", "pcmanfm"]
layers         = ["rofi", "wofi"]
layer_contains = "launcher"

# Example: force Russian for messaging apps
# [[force_layout.rules]]
# layout = "ru"
# apps   = ["telegram-desktop", "discord"]

[general]
layout_file     = "/tmp/hypr-layout"
switch_delay_ms = 150
"#;
