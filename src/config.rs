//! Configuration: loading, defaults, writing the skeleton config file.

use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

// ── Default values ────────────────────────────────────────────────────────────

fn default_device() -> String {
    "keyd-virtual-keyboard".into()
}
fn default_layouts() -> Vec<String> {
    vec!["en".into(), "ru".into()]
}
fn default_modifier() -> String {
    "Meta".into()
}
fn default_key() -> String {
    "Space".into()
}
fn default_osd_enabled() -> bool {
    true
}
fn default_osd_icon() -> String {
    "input-keyboard-symbolic".into()
}
fn default_layer_contains() -> String {
    "launcher".into()
}
fn default_layout_file() -> String {
    "/tmp/hypr-layout".into()
}
fn default_switch_delay_ms() -> u64 {
    150
}

// ── Sub-tables ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeyboardConfig {
    #[serde(default = "default_device")]
    pub device: String,
    #[serde(default = "default_layouts")]
    pub layouts: Vec<String>,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            device: default_device(),
            layouts: default_layouts(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    #[serde(default = "default_modifier")]
    pub modifier: String,
    #[serde(default = "default_key")]
    pub key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifier: default_modifier(),
            key: default_key(),
        }
    }
}

/// Per-layout OSD messages: TOML table keyed by layout name,
/// e.g. `[osd.messages]  en = "🇺🇸 US English"`.
pub type OsdMessages = std::collections::HashMap<String, String>;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OsdConfig {
    #[serde(default = "default_osd_enabled")]
    pub enabled: bool,
    #[serde(default = "default_osd_icon")]
    pub icon: String,
    #[serde(default)]
    pub messages: OsdMessages,
}

impl Default for OsdConfig {
    fn default() -> Self {
        let mut messages = std::collections::HashMap::new();
        messages.insert("en".into(), "🇺🇸 US English".into());
        messages.insert("ru".into(), "🇷🇺 Russian".into());
        Self {
            enabled: default_osd_enabled(),
            icon: default_osd_icon(),
            messages,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AutoSwitchConfig {
    pub english_apps: Vec<String>,
    pub english_layers: Vec<String>,
    #[serde(default = "default_layer_contains")]
    pub english_layer_contains: String,
}

impl Default for AutoSwitchConfig {
    fn default() -> Self {
        Self {
            english_apps: vec![
                "nvim".into(),
                "vim".into(),
                "btop".into(),
                "htop".into(),
                "alacritty".into(),
                "foot".into(),
                "kitty".into(),
                "mpv".into(),
                "pcmanfm".into(),
            ],
            english_layers: vec!["rofi".into(), "wofi".into()],
            english_layer_contains: default_layer_contains(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    #[serde(default = "default_layout_file")]
    pub layout_file: String,
    #[serde(default = "default_switch_delay_ms")]
    pub switch_delay_ms: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            layout_file: default_layout_file(),
            switch_delay_ms: default_switch_delay_ms(),
        }
    }
}

// ── Top-level Config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub keyboard:    KeyboardConfig,
    pub hotkey:      HotkeyConfig,
    pub osd:         OsdConfig,
    pub auto_switch: AutoSwitchConfig,
    pub general:     GeneralConfig,
}

impl Config {
    /// Load from file; on any error fall back to compiled-in defaults and log.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Err(e) => {
                eprintln!("[config] cannot read {:?}: {} — using defaults", path, e);
                Self::default()
            }
            Ok(text) => match toml::from_str::<Self>(&text) {
                Ok(cfg) => {
                    eprintln!("[config] loaded: {:?}", path);
                    cfg
                }
                Err(e) => {
                    eprintln!("[config] parse error in {:?}: {} — using defaults", path, e);
                    Self::default()
                }
            },
        }
    }

    /// Return the OSD message for a given layout name (falls back to the name
    /// itself so there is always something to display).
    pub fn osd_message<'a>(&'a self, layout: &'a str) -> &'a str {
        self.osd
            .messages
            .get(layout)
            .map(String::as_str)
            .unwrap_or(layout)
    }

    /// Index of `layout` in the configured rotation, or `None`.
    pub fn layout_index(&self, layout: &str) -> Option<usize> {
        self.keyboard.layouts.iter().position(|l| l == layout)
    }

    /// True if the window class should force the first layout.
    pub fn is_english_class(&self, cls: &str) -> bool {
        self.auto_switch.english_apps.iter().any(|a| a == cls)
    }

    /// True if the layer surface name requires the first layout.
    pub fn is_english_layer(&self, name: &str) -> bool {
        if self.auto_switch.english_layers.iter().any(|l| l == name) {
            return true;
        }
        let sub = &self.auto_switch.english_layer_contains;
        !sub.is_empty() && name.contains(sub.as_str())
    }

    /// Write a skeleton config to `path` **only if the file does not exist**.
    pub fn write_default(path: &Path) {
        if path.exists() {
            return;
        }
        if let Some(dir) = path.parent() {
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!("[config] mkdir {:?}: {}", dir, e);
                return;
            }
        }
        let content = r#"# hyprxkb configuration
# Full reference: https://github.com/blackcityskyline/hyprxkb

[keyboard]
device  = "keyd-virtual-keyboard"
# Layout rotation order when using the hotkey.
layouts = ["en", "ru"]

[hotkey]
# Modifier: Meta (Super/Win), Alt, Ctrl, Shift
modifier = "Meta"
# Key: Space, Tab, F1..F12, Grave, Minus, Equal, Left, Right, Up, Down
key = "Space"

[osd]
enabled = true
icon    = "input-keyboard-symbolic"

[osd.messages]
en = "🇺🇸 US English"
ru = "🇷🇺 Russian"

[auto_switch]
# Window classes that force the first layout.
english_apps = [
    "nvim", "vim", "btop", "htop",
    "alacritty", "foot", "kitty", "mpv", "pcmanfm",
]
# Layer surfaces by exact name.
english_layers = ["rofi", "wofi"]
# Layer surfaces whose name *contains* this substring.
english_layer_contains = "launcher"

[general]
layout_file    = "/tmp/hypr-layout"
switch_delay_ms = 150
"#;
        match fs::write(path, content) {
            Ok(()) => eprintln!("[config] created default: {:?}", path),
            Err(e) => eprintln!("[config] write {:?}: {}", path, e),
        }
    }

    /// Resolve the config path from $HOME.
    pub fn default_path() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join(".config")
                .join("hyprxkb")
                .join("config.toml")
        })
    }
}
