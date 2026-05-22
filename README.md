# hyprxkb

Keyboard layout switcher for [Hyprland](https://hyprland.org/).

## Features

- **Auto-switch** — force a layout when specific apps or layer surfaces are active; restore previous layout on exit
- **Hotkey** — cycle layouts with a configurable `Super+key`, works on the lock screen via raw `/dev/input`
- **Per-window memory** — remember last layout per app and restore on focus (like Windows/macOS IME)
- **Notifications** — `swayosd`, `notify-send`, [QuickShell](https://quickshell.outfoxxed.me/) toast layers, or none
- **CapsLock monitor** — notification on CapsLock state change
- **Waybar integration** — instant bar refresh via SIGRTMIN signal
- **Glob patterns** — `*` and `?` wildcards in app class rules
- **External sync** — detects layout changes made by other tools and stays consistent
- **CLI** — `init`, `reload`, `status [--json]`, `switch <layout>`, `list`

## Build

```bash
cargo build --release
# Binary: target/release/hyprxkb
```

### Runtime dependencies

| Tool | Required | Purpose |
|---|---|---|
| `hyprctl` | yes | Bundled with Hyprland |
| `swayosd-client` | no | OSD overlays |
| `notify-send` | no | dunst / mako / swaync |
| QuickShell with IPC socket | no | Toast layer notifications |

**`/dev/input` access** is required for the lock-screen hotkey.
Add yourself to the `input` group, or install a udev rule:

```udev
# /etc/udev/rules.d/70-input.rules
KERNEL=="event*", GROUP="input", MODE="0660"
```

## Installation

```bash
# Build
cargo build --release

# Copy binary
install -Dm755 target/release/hyprxkb ~/.local/bin/hyprxkb

# Generate default config
hyprxkb init
```

### systemd user service

```bash
# Copy the service file
install -Dm644 hyprxkb.service ~/.config/systemd/user/hyprxkb.service

systemctl --user daemon-reload
systemctl --user enable --now hyprxkb
```

### Hyprland autostart (alternative)

```ini
# ~/.config/hypr/hyprland.conf
exec-once = hyprxkb
```

## Usage

```bash
hyprxkb              # start
hyprxkb init         # write default config to ~/.config/hyprxkb/config.toml
hyprxkb reload       # reload config (sends SIGUSR1 to running instance)
hyprxkb status       # print current layout label (plain text)
hyprxkb status --json  # print JSON — for waybar return-type: json
hyprxkb switch ru    # switch to a specific XKB layout
hyprxkb list         # list all configured layouts
```

## Configuration

Default path: `~/.config/hyprxkb/config.toml`  
Run `hyprxkb init` to generate it.

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

Set `waybar_signal = 8` in `[notify]`. The bar refreshes the moment the layout changes.

## QuickShell integration

Set `backend = "quickshell"` in `[notify]`. On every layout switch hyprxkb sends
a JSON message to the QS IPC socket:

```json
{"type": "layout", "label": "🇷🇺 Russian"}
{"type": "capslock", "enabled": true}
```

The socket path defaults to `$XDG_RUNTIME_DIR/quickshell.sock` and can be
overridden with `quickshell_socket` in the config.

## Force-layout rules

Rules match window classes and layer surfaces. The first matching rule wins.

```toml
[[force_layout.rules]]
layout = "us"

# Window class patterns — case-insensitive, * and ? wildcards supported.
apps = ["nvim", "org.telegram.*", "*term*"]

# Exact layer surface names.
layers = ["rofi", "wofi"]

# Substring match for layer surface names.
layer_contains = ["launcher", "runner"]
```

When a matching window gains focus, the specified layout is forced and the
previous layout is saved. It is restored when a non-matching window gains focus
or when the layer surface is closed.

## Per-window memory

With `per_window_memory = true`, hyprxkb remembers the last layout you used in
each app (by window class) and restores it when that app regains focus. Works
like the IME behaviour on Windows and macOS. Force-layout rules take priority
over window memory.

## Migration from v0.3

| v0.3 | v0.4 |
|---|---|
| `[messages]` | `[labels]` |
| `general.layout_file` | `general.state_file` |
| `device = "keyd-virtual-keyboard"` | `device = "all"` (recommended) |
| `layer_contains = "launcher"` | `layer_contains = ["launcher"]` |
| `modifier = "Meta"` | `modifier = "Super"` (Meta still works) |
| `layouts = ["en", "ru"]` | `layouts = ["us", "ru"]` (strict XKB ids) |
| PID at `/tmp/hyprxkb.pid` | PID at `$XDG_RUNTIME_DIR/hyprxkb.pid` |
