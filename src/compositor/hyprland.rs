//! Hyprland compositor implementation.
//!
//! - Events: reads socket2 (event socket) line-by-line.
//! - Layout set: `hyprctl switchxkblayout <device> <index>`.
//! - Layout query: `hyprctl -j getoption input:kb_layout` (parses JSON).

use super::{Compositor, CompositorEvent};
use std::{
    env,
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::Command,
};

// ---------------------------------------------------------------------------
// Socket path
// ---------------------------------------------------------------------------

fn socket2_path() -> Result<PathBuf, String> {
    let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| "HYPRLAND_INSTANCE_SIGNATURE is not set — is Hyprland running?".to_string())?;
    let xdg = env::var("XDG_RUNTIME_DIR")
        .map_err(|_| "XDG_RUNTIME_DIR is not set".to_string())?;
    Ok(PathBuf::from(xdg).join("hypr").join(sig).join(".socket2.sock"))
}

// ---------------------------------------------------------------------------
// Event parsing
// ---------------------------------------------------------------------------

fn parse_line(line: &str) -> Option<CompositorEvent> {
    // activewindow>><class>,<title> — title is ignored
    if let Some(rest) = line.strip_prefix("activewindow>>") {
        let class = rest.split_once(',').map_or(rest, |(c, _)| c);
        return Some(CompositorEvent::WindowFocus {
            class: class.trim().to_ascii_lowercase(),
        });
    }
    // openlayer>><name>
    if let Some(name) = line.strip_prefix("openlayer>>") {
        return Some(CompositorEvent::LayerOpen {
            name: name.trim().to_ascii_lowercase(),
        });
    }
    // closelayer>><name>
    if let Some(name) = line.strip_prefix("closelayer>>") {
        return Some(CompositorEvent::LayerClose {
            name: name.trim().to_ascii_lowercase(),
        });
    }
    None
}

// ---------------------------------------------------------------------------
// HyprlandCompositor
// ---------------------------------------------------------------------------

pub struct HyprlandCompositor {
    reader: BufReader<UnixStream>,
    buf:    String,
}

impl HyprlandCompositor {
    /// Connect to the Hyprland event socket. Blocks until the socket is available.
    pub fn connect() -> Result<Self, String> {
        let path = socket2_path()?;
        let stream = UnixStream::connect(&path)
            .map_err(|e| format!("cannot connect to Hyprland socket {path:?}: {e}"))?;
        eprintln!("[hyprland] connected to {path:?}");
        Ok(Self {
            reader: BufReader::new(stream),
            buf:    String::new(),
        })
    }
}

impl Compositor for HyprlandCompositor {
    fn next_event(&mut self) -> Option<CompositorEvent> {
        loop {
            self.buf.clear();
            match self.reader.read_line(&mut self.buf) {
                Ok(0)  => {
                    eprintln!("[hyprland] socket closed (EOF)");
                    return None;
                }
                Err(e) => {
                    eprintln!("[hyprland] socket read error: {e}");
                    return None;
                }
                Ok(_) => {}
            }
            let line = self.buf.trim_end_matches(['\n', '\r']);
            if let Some(ev) = parse_line(line) {
                return Some(ev);
            }
            // Unknown event — skip silently.
        }
    }

    fn set_layout(&self, device: &str, index: usize) -> Result<(), String> {
        let out = Command::new("hyprctl")
            .args(["switchxkblayout", device, &index.to_string()])
            .output()
            .map_err(|e| format!("hyprctl: {e}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!("hyprctl switchxkblayout failed: {stderr}"));
        }
        Ok(())
    }

    /// Query the current layout index from hyprctl and map it back to a name.
    ///
    /// `hyprctl -j activelayout` returns something like:
    /// `[{"device":"...","layout":"English (US)"}]`
    /// We match by device name and return the layout string, which we then
    /// cross-reference against our config's layout list.
    ///
    /// NOTE: Hyprland reports the *full* layout name (e.g. "English (US)"),
    /// not the XKB identifier (e.g. "us"). We cannot reliably reverse-map
    /// this without a lookup table. Instead, we store the XKB index via
    /// `hyprctl getoption input:kb_layout` which returns the raw TOML value.
    ///
    /// For now we return `None` — the engine will skip the sync step rather
    /// than guess. A future version can implement the full mapping.
    fn active_layout(&self, device: &str) -> Option<String> {
        // `hyprctl -j activelayout` returns an array of {device, layout}.
        let out = Command::new("hyprctl")
            .args(["-j", "activelayout"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = std::str::from_utf8(&out.stdout).ok()?;
        // Minimal JSON scan — avoid pulling in a full parser for a tiny struct.
        // Format: [{"device":"NAME","layout":"LAYOUT"}, ...]
        for block in text.split('{') {
            if block.contains(device) {
                // Extract "layout":"VALUE"
                if let Some(after) = block.find("\"layout\"") {
                    let rest = &block[after + 8..]; // skip `"layout"`
                    if let Some(start) = rest.find('"') {
                        let inner = &rest[start + 1..];
                        if let Some(end) = inner.find('"') {
                            return Some(inner[..end].to_owned());
                        }
                    }
                }
            }
        }
        None
    }
}
