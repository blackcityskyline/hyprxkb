//! Configuration schema: all structs, defaults, serde impls.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Notify
// ---------------------------------------------------------------------------

/// Which notification backend to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NotifyBackend {
    /// No notifications.
    #[default]
    None,
    /// `swayosd-client` — OSD overlays (swayosd).
    SwayOsd,
    /// `notify-send` — works with dunst, mako, swaync, libnotify.
    NotifySend,
    /// Noctalia Shell / Quickshell toast layers (qs IPC).
    /// Sends a JSON payload to the QS IPC socket.
    QuickShell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifyConfig {
    /// Backend to use for layout-switch notifications.
    pub backend: NotifyBackend,
    /// Notification timeout in milliseconds (notify-send / quickshell).
    pub timeout_ms: u64,
    /// Icon name for notify-send and swayosd.
    pub icon: String,
    /// If set, send `pkill -SIGRTMIN+N waybar` after every layout switch
    /// so the custom bar module refreshes instantly.
    /// Example: `waybar_signal = 8`
    pub waybar_signal: Option<u8>,
    /// QuickShell IPC socket path.
    /// Defaults to `$XDG_RUNTIME_DIR/quickshell.sock`.
    pub quickshell_socket: Option<String>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            backend:           NotifyBackend::None,
            timeout_ms:        2000,
            icon:              "input-keyboard-symbolic".into(),
            waybar_signal:     None,
            quickshell_socket: None,
        }
    }
}

// ---------------------------------------------------------------------------
// CapsLock
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CapsLockConfig {
    /// Enable CapsLock state monitoring.
    pub enabled: bool,
    /// How often to poll `/sys/class/leds/` (milliseconds).
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardConfig {
    /// The hyprctl device name (see `hyprctl devices`).
    /// Use `*` to match all keyboards (applies to first detected).
    pub device: String,
    /// Layout rotation order.
    /// Names must be valid XKB layout identifiers (e.g. `us`, `ru`, `de`, `gb`).
    /// These are passed verbatim to `hyprctl switchxkblayout`.
    pub layouts: Vec<String>,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            device:  "all".into(),
            layouts: vec!["us".into(), "ru".into()],
        }
    }
}

// ---------------------------------------------------------------------------
// Hotkey
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    /// Modifier key: `Super` | `Alt` | `Ctrl` | `Shift`
    pub modifier: String,
    /// Trigger key: `Space` | `Tab` | `Grave` | `Minus` | `F1`–`F12` | …
    pub key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifier: "Super".into(),
            key:      "Space".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Force-layout rules
// ---------------------------------------------------------------------------

/// A rule that forces a specific layout when matched windows/layers are active.
///
/// Rules are evaluated in declaration order; the first match wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceRule {
    /// XKB layout to force (must be in `keyboard.layouts`).
    pub layout: String,

    /// Window class patterns to match against.
    /// Supports `*` (any sequence) and `?` (any single char) wildcards.
    /// Matching is case-insensitive.
    #[serde(default)]
    pub apps: Vec<String>,

    /// Exact layer surface names that trigger this rule (e.g. `rofi`, `wofi`).
    #[serde(default)]
    pub layers: Vec<String>,

    /// Substring match for layer surface names (e.g. `launcher`).
    #[serde(default)]
    pub layer_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ForceLayoutConfig {
    pub rules: Vec<ForceRule>,
}

impl Default for ForceLayoutConfig {
    fn default() -> Self {
        Self {
            rules: vec![ForceRule {
                layout: "us".into(),
                apps: vec![
                    "org.alacritty".into(),
                    "foot".into(),
                    "kitty".into(),
                    "org.wezfurlong.wezterm".into(),
                    "nvim".into(),
                    "vim".into(),
                    "btop".into(),
                    "htop".into(),
                    "mpv".into(),
                ],
                layers:         vec!["rofi".into(), "wofi".into()],
                layer_contains: vec!["launcher".into(), "runner".into()],
            }],
        }
    }
}

// ---------------------------------------------------------------------------
// General
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// File used to persist the current layout across restarts.
    pub state_file: String,
    /// Minimum time (ms) between automatic layout switches triggered by window
    /// focus events. Does NOT affect manual hotkey switches.
    pub switch_delay_ms: u64,
    /// Remember the last layout per app class and restore it on focus.
    /// Mirrors Windows/macOS IME behaviour. Disabled by default.
    pub per_window_memory: bool,
    /// How often (ms) to sync the real compositor layout into our state,
    /// so external layout changes (e.g. from another tool) don't desync us.
    pub sync_interval_ms: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            state_file:        "/tmp/hyprxkb-state".into(),
            switch_delay_ms:   100,
            per_window_memory: false,
            sync_interval_ms:  5000,
        }
    }
}

// ---------------------------------------------------------------------------
// Root config
// ---------------------------------------------------------------------------

/// Human-readable display names shown in notifications and status output.
/// Key: XKB layout identifier (e.g. `"us"`), value: display string (e.g. `"🇺🇸 English"`).
pub type LayoutLabels = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub keyboard:     KeyboardConfig,
    pub hotkey:       HotkeyConfig,
    pub notify:       NotifyConfig,
    pub capslock:     CapsLockConfig,
    pub force_layout: ForceLayoutConfig,
    pub general:      GeneralConfig,
    /// Display labels for each layout.
    pub labels: LayoutLabels,
}

impl Config {
    /// Human-readable label for `layout` (falls back to the XKB identifier).
    pub fn label<'a>(&'a self, layout: &'a str) -> &'a str {
        self.labels.get(layout).map(String::as_str).unwrap_or(layout)
    }

    /// Index of `layout` in `keyboard.layouts`.
    /// This is the index passed to `hyprctl switchxkblayout`.
    pub fn layout_index(&self, layout: &str) -> Option<usize> {
        self.keyboard.layouts.iter().position(|l| l == layout)
    }

    /// Validate the config and return a list of human-readable warnings.
    pub fn warnings(&self) -> Vec<String> {
        let mut w = Vec::new();
        if self.keyboard.layouts.is_empty() {
            w.push("keyboard.layouts is empty — nothing to switch".into());
        }
        for rule in &self.force_layout.rules {
            if self.layout_index(&rule.layout).is_none() {
                w.push(format!(
                    "force_layout rule references unknown layout {:?} \
                     (not in keyboard.layouts)",
                    rule.layout
                ));
            }
        }
        w
    }
}
