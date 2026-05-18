# hyprkbtool

Keyboard layout-switching daemon for [Hyprland](https://hyprland.org/).

## Features

* **Auto-switch**: force the first layout for configured apps and layer surfaces
* **Restore**: return to the previous layout when leaving a forced context
* **Hotkey**: cycle layouts with a configurable modifier+key combo (works on the lock screen via raw `/dev/input`)
* **OSD**: optional notifications via `swayosd-client`

## Build

```bash
# Release (minimal binary ~200 KB with LTO+strip)
cargo build --release

# The binary is at target/release/hyprkbtool
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

Default location: `~/.config/hyprkbtool/config.toml`  
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
exec-once = hyprkbtool
```

## Bug fixes vs the original C version

1. **Throttle timer** — in C, `last_switch_ms` was stamped on *every* `activewindow` event, even no-ops (non-English app with no saved layout).  In Rust, the timer is only updated when a layout change is actually triggered.

2. **Save-slot overwrite** — in C, every `save_and_set_first` call unconditionally overwrote `g_saved_layout`, so switching between two English apps lost the original non-English layout.  In Rust, `save_and_set_first` only writes the slot the *first* time (while `saved_layout.is_none()`).

3. **evdev SYNC handling** — in C, `LIBEVDEV_READ_STATUS_SYNC` was handled with the same `LIBEVDEV_READ_FLAG_NORMAL` flag, causing the event buffer to desynchronise.  The Rust `evdev` crate's `fetch_events()` handles the SYNC drain internally.

4. **Blocking OSD** — in C, `osd_notify` called `waitpid` in the calling thread, blocking the event loop until `swayosd-client` exited.  In Rust, `Command::spawn()` returns immediately; the child is reaped by the OS when it exits.

5. **`last_switch_ms` without mutex** — in C this global was read/written from both the main thread and `handle_layer_*` without the lock.  In Rust the timer lives only in `main` (which owns the event loop) so no sharing is needed.
