//! Hyprland IPC: socket2 connection and line-by-line event parsing.

use std::{
    env,
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
    path::PathBuf,
};

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

fn socket_path() -> Result<PathBuf, String> {
    let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| "HYPRLAND_INSTANCE_SIGNATURE not set".to_string())?;
    let xdg = env::var("XDG_RUNTIME_DIR")
        .map_err(|_| "XDG_RUNTIME_DIR not set".to_string())?;
    Ok(PathBuf::from(xdg).join("hypr").join(sig).join(".socket2.sock"))
}

pub fn connect() -> Result<BufReader<UnixStream>, String> {
    let path = socket_path()?;
    let stream = UnixStream::connect(&path)
        .map_err(|e| format!("connect {path:?}: {e}"))?;
    Ok(BufReader::new(stream))
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HyprEvent {
    ActiveWindow { class: String, #[allow(dead_code)] title: String },
    LayerOpen(String),
    LayerClose(String),
}

fn parse_line(line: &str) -> Option<HyprEvent> {
    if let Some(rest) = line.strip_prefix("activewindow>>") {
        let (class, title) = rest.split_once(',').unwrap_or((rest, ""));
        return Some(HyprEvent::ActiveWindow {
            class: class.to_ascii_lowercase(),
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

// ---------------------------------------------------------------------------
// Iterator adapter
// ---------------------------------------------------------------------------

pub struct EventStream<R: BufRead> {
    reader: R,
    buf:    String,
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
                Ok(0)  => return None,
                Err(e) => { eprintln!("[hypr] read error: {e}"); return None; }
                Ok(_)  => {}
            }
            let line = self.buf.trim_end_matches(['\n', '\r']);
            if let Some(ev) = parse_line(line) {
                return Some(ev);
            }
        }
    }
}
