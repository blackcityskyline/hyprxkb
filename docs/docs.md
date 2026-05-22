# Configuration

Default path: `~/.config/hyprxkb/config.toml`  
Run `hyprxkb init` to generate it.

---

## [keyboard]

```toml
[keyboard]
device  = "all"
layouts = ["us", "ru"]
```

| Key | Default | Description |
|---|---|---|
| `device` | `"all"` | Device name from `hyprctl devices`. `"all"` switches every keyboard simultaneously — useful when keyd or other virtual keyboards are present. |
| `layouts` | `["us", "ru"]` | Layout rotation order. Must be valid XKB identifiers: `us`, `ru`, `de`, `gb`, `fr`, … |

---

## [hotkey]

```toml
[hotkey]
modifier = "Super"
key      = "Space"
```

| Key | Default | Values |
|---|---|---|
| `modifier` | `"Super"` | `Super` `Alt` `Ctrl` `Shift` |
| `key` | `"Space"` | `Space` `Tab` `Grave` `Minus` `Equal` `F1`–`F12` `Left` `Right` `Up` `Down` |

The hotkey is read from raw `/dev/input`, so it works on the lock screen. Requires access to the `input` group.

---

## [labels]

Human-readable names shown in notifications and `hyprxkb status`.  
Keys must match XKB identifiers from `keyboard.layouts`.

```toml
[labels]
us = "🇺🇸 English" # or any custom label
ru = "🇷🇺 Russian"
de = "🇩🇪 Deutsch"
```

If a label is not defined, the XKB identifier is used as a fallback.

---

## [notify]

```toml
[notify]
backend           = "none"
timeout_ms        = 2000
icon              = "input-keyboard-symbolic"
waybar_signal     = 8
# quickshell_socket = "/run/user/1000/quickshell.sock"
```

| Key | Default | Description |
|---|---|---|
| `backend` | `"none"` | `none` · `swayosd` · `notify-send` · `quickshell` |
| `timeout_ms` | `2000` | Notification duration in ms (`notify-send` and `quickshell`) |
| `icon` | `"input-keyboard-symbolic"` | Icon name (`notify-send` and `swayosd`) |
| `waybar_signal` | — | If set, sends `SIGRTMIN+N` to waybar after every switch. Must match the `signal` field in your waybar module. |
| `quickshell_socket` | `$XDG_RUNTIME_DIR/quickshell.sock` | Override the QuickShell IPC socket path |

### Backends

**`swayosd`** — calls `swayosd-client --custom-message <label>`. CapsLock uses `--caps-lock`.

**`notify-send`** — calls `notify-send` with urgency `low`. Works with dunst, mako, swaync, and any libnotify-compatible daemon.

**`quickshell`** — sends a JSON payload over a Unix socket to a QuickShell IPC listener:

```json
{ "type": "layout",   "label": "🇷🇺 Russian" }
{ "type": "capslock", "enabled": true }
```

---

## [capslock]

```toml
[capslock]
enabled = true
poll_ms = 150
```

| Key | Default | Description |
|---|---|---|
| `enabled` | `true` | Monitor `/sys/class/leds/` for CapsLock state changes |
| `poll_ms` | `150` | Polling interval in ms |

---

## [general]

```toml
[general]
state_file        = "/tmp/hyprxkb-state"
switch_delay_ms   = 100
per_window_memory = false
sync_interval_ms  = 5000
```

| Key | Default | Description |
|---|---|---|
| `state_file` | `"/tmp/hyprxkb-state"` | File used to persist the active layout across restarts |
| `switch_delay_ms` | `100` | Minimum delay between automatic switches triggered by window focus events. Does not affect the hotkey. |
| `per_window_memory` | `false` | Remember the last layout per app class and restore it on focus. Works like IME behaviour on Windows and macOS. Force-layout rules take priority. |
| `sync_interval_ms` | `5000` | How often (ms) to query the compositor for the active layout. Keeps state consistent when another tool changes the layout externally. |

---

## [[force_layout.rules]]

Force a specific layout when matching windows or layer surfaces are active.  
Rules are evaluated top-to-bottom; the first match wins.

```toml
[[force_layout.rules]]
layout = "us"
apps   = ["nvim", "vim", "vscode", "alacritty", "*term*"]
layers = ["rofi", "wofi", "fuzzel"]
layer_contains = ["launcher", "runner"]
```

| Key | Description |
|---|---|
| `layout` | XKB layout to activate. Must be in `keyboard.layouts`. |
| `apps` | Window class patterns. Case-insensitive. Supports `*` (any sequence) and `?` (any single character). Check window classes with `hyprctl clients`. |
| `layers` | Exact layer surface names. Check active layers with `hyprctl layers`. |
| `layer_contains` | Substring matches for layer surface names. |

When a matching window gains focus, the layout is forced and the previous layout is saved. It is restored when a non-matching window gains focus or the layer closes.

Multiple rules can be stacked:

```toml
[[force_layout.rules]]
layout = "us"
apps   = ["nvim", "foot", "kitty"]

[[force_layout.rules]]
layout = "ru"
apps   = ["org.telegram.desktop", "discord"]
```
