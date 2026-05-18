# hyprxkb

Keyboard layout-switching utility for [Hyprland](https://hyprland.org/).

## Features

* **Auto-switch**: force the first layout for configured apps and layer surfaces
* **Restore**: return to the previous layout when leaving a forced context
* **Hotkey**: cycle layouts with a configurable modifier+key combo (works on the lock screen via raw `/dev/input`)
* **OSD**: optional notifications via `swayosd-client`

## Build

```bash
# Release (minimal binary ~200 KB with LTO+strip)
cargo build --release

# The binary is at target/release/hyprxkb
```

### Runtime dependencies

| Tool | Required | Notes |
|------|----------|-------|
| `hyprctl` | yes | Ships with Hyprland |
| `swayosd-client` | no | OSD — disable with `osd.enabled = false` |

`/dev/input/event*` access is needed for the lock-screen hotkey.  Either run as root, add yourself to the `input` group, or use a udev rule:

```udev
# /etc/udev/rules.d/70-input-group.rules
KERNEL=="event*", GROUP="input", MODE="0660"
```

## Config

Default location: `~/.config/hyprxkb/config.toml`  
Created automatically on first run.

```toml
[keyboard]
device  = "keyd-virtual-keyboard"
layouts = ["en", "ru"]          # rotation order

[hotkey]
modifier = "Meta"               # Meta | Alt | Ctrl | Shift
key      = "Space"

[osd]
enabled = true
icon    = "input-keyboard-symbolic"

[osd.messages]
en = "🇺🇸 US English"
ru = "🇷🇺 Russian"

[auto_switch]
english_apps            = ["nvim", "vim", "alacritty", "foot", "kitty"]
english_layers          = ["rofi", "wofi"]
english_layer_contains  = "launcher"

[general]
layout_file     = "/tmp/hypr-layout"
switch_delay_ms = 150
```

## Hyprland autostart

```ini
# ~/.config/hypr/hyprland.conf
exec-once = hyprxkb
```
