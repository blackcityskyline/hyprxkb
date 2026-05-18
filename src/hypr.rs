//! Hyprland IPC: socket2 connection, line-by-line event loop.

use std::{
    env,
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
    path::PathBuf,
};

/// Resolve the path to Hyprland's `.socket2.sock`.
pub fn socket_path() -> Result<PathBuf, String> {
    let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| "HYPRLAND_INSTANCE_SIGNATURE not set".to_string())?;
    let xdg = env::var("XDG_RUNTIME_DIR")
        .map_err(|_| "XDG_RUNTIME_DIR not set".to_string())?;
    Ok(PathBuf::from(xdg).join("hypr").join(sig).join(".socket2.sock"))
}

/// Connect to the socket and return a `BufReader` over it.
pub fn connect() -> Result<BufReader<UnixStream>, String> {
    let path = socket_path()?;
    let stream = UnixStream::connect(&path)
        .map_err(|e| format!("connect {:?}: {}", path, e))?;
    Ok(BufReader::new(stream))
}

// ── Event types ───────────────────────────────────────────────────────────────

/// Parsed Hyprland socket2 events we care about.
#[derive(Debug)]
pub enum HyprEvent {
    /// `activewindow>>class,title`
    ActiveWindow { class: String, #[allow(dead_code)] title: String },
    /// `openlayer>>name`
    LayerOpen(String),
    /// `closelayer>>name`
    LayerClose(String),
}

/// Parse a single raw line into a `HyprEvent`, or return `None` for unknown
/// events.
pub fn parse_line(line: &str) -> Option<HyprEvent> {
    // BUG FIX vs C: use str::split_once for robust parsing instead of manual
    // pointer arithmetic that breaks if the prefix length is wrong.
    if let Some(rest) = line.strip_prefix("activewindow>>") {
        // rest = "class,title" (title may itself contain commas)
        let (cls, title) = rest.split_once(',').unwrap_or((rest, ""));
        return Some(HyprEvent::ActiveWindow {
            class: cls.to_ascii_lowercase(),
            title: title.to_owned(),
        });
    }
    if let Some(name) = line.strip_prefix("openlayer>>") {
        return Some(HyprEvent::LayerOpen(name.to_owned()));
    }
    if let Some(name) = line.strip_prefix("closelayer>>") {
        return Some(HyprEvent::LayerClose(name.to_owned()));
    }
    None
}

/// Iterator over `HyprEvent`s from a buffered socket reader.
pub struct EventStream<R: BufRead> {
    reader: R,
    buf: String,
}

impl<R: BufRead> EventStream<R> {
    pub fn new(reader: R) -> Self {
        Self { reader, buf: String::new() }
    }
}

impl<R: BufRead> Iterator for EventStream<R> {
    type Item = HyprEvent;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.buf.clear();
            match self.reader.read_line(&mut self.buf) {
                Ok(0) => return None, // EOF
                Err(e) => {
                    eprintln!("[hypr] read error: {}", e);
                    return None;
                }
                Ok(_) => {}
            }
            let line = self.buf.trim_end_matches('\n').trim_end_matches('\r');
            if let Some(ev) = parse_line(line) {
                return Some(ev);
            }
            // Unknown event — keep reading.
        }
    }
}
