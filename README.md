# hyprxkb

Keyboard layout-switching daemon for [Hyprland](https://hyprland.org/).

## Features

- **Auto-switch** — force a layout when specific apps or layer surfaces are active
- **Restore** — return to the previous layout when leaving a forced context
- **Hotkey** — cycle layouts with a configurable `modifier+key` (works on the lock screen via raw `/dev/input`)
- **Notifications** — optional OSD via `swayosd` or any `notify-send` compatible daemon
- **CapsLock monitor** — send a notification on CapsLock state change
- **CLI** — `reload`, `status`, `switch <layout>` subcommands

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
| `notify-send` | no | For dunst / mako / swaync |

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
hyprxkb status           # print current layout name
hyprxkb switch ru        # switch to a specific layout
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
backend    = "none"
timeout_ms = 2000
icon       = "input-keyboard-symbolic"

# CapsLock state monitoring.
[capslock]
enabled = true
poll_ms = 150

# Force a layout when certain apps or layers are active.
# Multiple rules are evaluated in order — first match wins.
[[force_layout.rules]]
layout         = "en"
apps           = ["nvim", "vim", "btop", "htop", "alacritty", "foot", "kitty"]
layers         = ["rofi", "wofi"]
layer_contains = "launcher"

[general]
layout_file     = "/tmp/hypr-layout"
switch_delay_ms = 150
```

## Hyprland autostart

```ini
# ~/.config/hypr/hyprland.conf
exec-once = hyprxkb
```

## Waybar integration

Use `hyprxkb status` as a custom module:

```json
"custom/layout": {
    "exec": "hyprxkb status",
    "interval": 2,
    "format": "⌨ {}"
}
```

---

## Ideas / Roadmap

- **Per-window layout memory** — remember the last layout for each app class and restore it on focus (like Windows/macOS IME behaviour)
- **Waybar signal** — send `pkill -SIGRTMIN+8 waybar` after every switch so the bar updates instantly
- **Glob/regex app matching** — `apps = ["*term*", "org.gnome.*"]` in force rules
- **`hyprxkb status --json`** — structured output for waybar `return-type: json` (layout, capslock, icon, tooltip)
- **Multi-device hotkeys** — separate hotkey per keyboard (e.g. laptop vs external)
