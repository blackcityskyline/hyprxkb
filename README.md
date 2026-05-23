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

> **Hotkey not working?** The lock-screen hotkey reads `/dev/input` directly.
> Add yourself to the `input` group and re-login:
> ```bash
> sudo usermod -aG input $USER
> ```
> Or install a udev rule: `KERNEL=="event*", GROUP="input", MODE="0660"` in `/etc/udev/rules.d/70-input.rules`

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
