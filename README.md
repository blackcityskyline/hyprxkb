# hyprxkb

Keyboard layout-switching daemon for [Hyprland](https://hyprland.org/).

## Features

- **Auto-switch** — force a layout when specific apps or layer surfaces are active
- **Restore** — return to the previous layout when leaving a forced context
- **Hotkey** — cycle layouts with a configurable `modifier+key` (works on the lock screen via raw `/dev/input`)
- **Notifications** — optional OSD via `swayosd` or any `notify-send` compatible daemon
- **CapsLock monitor** — notification on CapsLock state change
- **Per-window memory** — remember the last layout per app and restore it on focus
- **Waybar integration** — instant bar update via configurable SIGRTMIN signal
- **Glob matching** — `*` and `?` wildcards in app class patterns
- **CLI** — `reload`, `status [--json]`, `switch <layout>`, `list`

## Build

```bash
cargo build --release
# Binary: target/release/hyprxkb
```

### Runtime dependencies

| Tool | Required | Notes |
|---|---|---|
| `hyprctl` | yes | Bundled with Hyprland |
| `swayosd-client` | no | OSD overlays |
| `notify-send` | no | dunst / mako / swaync |

**`/dev/input` access** is needed for the lock-screen hotkey.
Add yourself to the `input` group, or drop a udev rule:

```udev
# /etc/udev/rules.d/70-input.rules
KERNEL=="event*", GROUP="input", MODE="0660"
```

## Usage

```bash
hyprxkb                  # start daemon
hyprxkb reload           # reload config (sends SIGUSR1)
hyprxkb status           # print current layout (plain text)
hyprxkb status --json    # print as JSON — for waybar return-type: json
hyprxkb switch ru        # switch to a specific layout
hyprxkb list             # list all configured layouts with display names
```

## Configuration

Default path: `~/.config/hyprxkb/config.toml` (created automatically on first run).

```toml
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

# Notification backend: none | swayosd | notify-send
[notify]
backend       = "none"
timeout_ms    = 2000
icon          = "input-keyboard-symbolic"
waybar_signal = 8   # optional: pkill -SIGRTMIN+8 waybar after each switch

# CapsLock state monitoring.
[capslock]
enabled = true
poll_ms = 150

# Force rules — app patterns support * and ? wildcards.
# Multiple rules evaluated in order; first match wins.
[[force_layout.rules]]
layout         = "en"
apps           = ["nvim", "vim", "*top", "alacritty", "foot", "kitty", "mpv"]
layers         = ["rofi", "wofi"]
layer_contains = "launcher"

[general]
layout_file       = "/tmp/hypr-layout"
switch_delay_ms   = 150
per_window_memory = false   # remember last layout per app (like Windows/macOS IME)
```

## Waybar integration

**Option A — plain text, polling:**
```json
"custom/layout": {
    "exec": "hyprxkb status",
    "interval": 2,
    "format": "⌨ {}"
}
```

**Option B — JSON with instant updates (recommended):**
```json
"custom/layout": {
    "exec": "hyprxkb status --json",
    "return-type": "json",
    "interval": "once",
    "signal": 8
}
```
Then set `waybar_signal = 8` in `[notify]`. The bar refreshes the moment the layout changes.

## Hyprland autostart

```ini
# ~/.config/hypr/hyprland.conf
exec-once = hyprxkb
```
