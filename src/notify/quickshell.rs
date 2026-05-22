//! QuickShell (Noctalia Shell / qs) notification backend.
//!
//! Sends a JSON payload over a Unix domain socket to the QS IPC listener.
//! The QS side should expose a toast layer that reads the payload.
//!
//! Payload format (layout changed):
//! ```json
//! {"type":"layout","label":"🇺🇸 English"}
//! ```
//!
//! Payload format (CapsLock):
//! ```json
//! {"type":"capslock","enabled":true}
//! ```
//!
//! The socket path defaults to `$XDG_RUNTIME_DIR/quickshell.sock`.

use super::{NotifyBackend, signal_waybar};
use crate::config::NotifyConfig;
use std::{
    env,
    io::Write,
    os::unix::net::UnixStream,
    path::PathBuf,
};

pub struct QuickShellBackend {
    socket_path:   PathBuf,
    waybar_signal: Option<u8>,
}

impl QuickShellBackend {
    pub fn new(cfg: &NotifyConfig) -> Self {
        let path = cfg.quickshell_socket.as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let xdg = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
                PathBuf::from(xdg).join("quickshell.sock")
            });
        Self {
            socket_path:   path,
            waybar_signal: cfg.waybar_signal,
        }
    }

    fn send(&self, payload: &str) {
        match UnixStream::connect(&self.socket_path) {
            Ok(mut stream) => {
                // Write payload + newline as a single message.
                let msg = format!("{payload}\n");
                if let Err(e) = stream.write_all(msg.as_bytes()) {
                    eprintln!("[quickshell] write error: {e}");
                }
            }
            Err(e) => {
                eprintln!(
                    "[quickshell] cannot connect to {:?}: {e}",
                    self.socket_path
                );
            }
        }
    }
}

impl NotifyBackend for QuickShellBackend {
    fn layout_changed(&self, label: &str) {
        // Escape label for JSON (handle quotes and backslashes).
        let escaped = label.replace('\\', "\\\\").replace('"', "\\\"");
        self.send(&format!(r#"{{"type":"layout","label":"{escaped}"}}"#));
        if let Some(sig) = self.waybar_signal {
            signal_waybar(sig);
        }
    }

    fn capslock_changed(&self, enabled: bool) {
        self.send(&format!(r#"{{"type":"capslock","enabled":{enabled}}}"#));
    }
}
