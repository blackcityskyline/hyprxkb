# hyprxkb

Keyboard layout switcher for [Hyprland](https://hyprland.org/).

Switches layouts per app and layer surface, remembers your last layout per window, works on the lock screen.

## Install

```bash
cargo build --release
install -Dm755 target/release/hyprxkb ~/.local/bin/hyprxkb
hyprxkb init   # generate ~/.config/hyprxkb/config.toml
```

Add to Hyprland:

```ini
exec-once = hyprxkb
```

Or as a systemd user service:

```bash
install -Dm644 hyprxkb.service ~/.config/systemd/user/hyprxkb.service
systemctl --user enable --now hyprxkb
```

> **`/dev/input` access** is needed for the lock-screen hotkey — add yourself to the `input` group or install a udev rule.

## Usage

```
hyprxkb              start
hyprxkb init         write default config
hyprxkb reload       reload config (SIGUSR1)
hyprxkb status       current layout (plain text)
hyprxkb status --json  current layout (JSON, for waybar)
hyprxkb switch ru    switch to layout
hyprxkb list         list all layouts
```

## Config

`~/.config/hyprxkb/config.toml` — example of config

```toml
[keyboard]
device  = "all"          # or a specific name from `hyprctl devices`
layouts = ["us", "ru"]   # XKB identifiers, rotation order

[hotkey]
modifier = "Super"
key      = "Space"

[labels]
us = "🇺🇸 English"

[notify]
backend = "none"   # none | swayosd | notify-send | quickshell
# waybar_signal = 8

[general]
per_window_memory = true
switch_delay_ms   = 100

[[force_layout.rules]]
layout = "us"
apps   = ["nvim", "vscode", "alacritty"] # check hyprctl clients
layers = ["rofi", "wofi"]                # check hyprctl layers
layer_contains = ["launcher"]
```

## Waybar

```json
"custom/layout": {
    "exec": "hyprxkb status --json",
    "return-type": "json",
    "interval": "once",
    "signal": 8
}
```

Set `waybar_signal = 8` in `[notify]`.

## Runtime dependencies

| | |
|---|---|
| `hyprctl` | bundled with Hyprland |
| `swayosd-client` | optional, for OSD notifications |
| `notify-send` | optional, dunst / mako / swaync |
| QuickShell | optional, for toast layer notifications |
