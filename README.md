# hyprxkb

Keyboard layout switcher for [Hyprland](https://hyprland.org/). Switches layouts per app and layer surface, remembers your last layout per window, works on the lock screen.

→ [Configuration reference](docs/docs.md)

## Install

```bash
cargo build --release
install -Dm755 target/release/hyprxkb ~/.local/bin/hyprxkb
hyprxkb init
```

```ini
# ~/.config/hypr/hyprland.conf
exec-once = hyprxkb
```

> `/dev/input` access is needed for the lock-screen hotkey — add yourself to the `input` group or install a udev rule.

## Usage

| Command | Description |
|---|---|
| `hyprxkb` | start |
| `hyprxkb init` | write default config |
| `hyprxkb reload` | reload config |
| `hyprxkb status` | current layout (plain text) |
| `hyprxkb status --json` | current layout (JSON, for waybar) |
| `hyprxkb switch <layout>` | switch to layout |
| `hyprxkb list` | list all layouts |

## Runtime dependencies

| | |
|---|---|
| `hyprctl` | bundled with Hyprland |
| `swayosd-client` | optional, OSD notifications |
| `notify-send` | optional, dunst / mako / swaync |
| QuickShell | optional, toast layer notifications |
