//! Notification backends.

use crate::config::{NotifyBackend, NotifyConfig};
use std::process::Command;

/// Send a generic layout-switch notification.
pub fn send(cfg: &NotifyConfig, summary: &str, body: &str) {
    match cfg.backend {
        NotifyBackend::None => {}

        NotifyBackend::SwayOsd => {
            Command::new("swayosd-client")
                .args(["--custom-message", summary, "--custom-icon", &cfg.icon])
                .spawn()
                .ok();
        }

        NotifyBackend::NotifySend => {
            Command::new("notify-send")
                .args([
                    "--icon",        &cfg.icon,
                    "--expire-time", &cfg.timeout_ms.to_string(),
                    "--urgency",     "low",
                    summary,
                    body,
                ])
                .spawn()
                .ok();
        }
    }
}

/// Send a CapsLock state notification.
pub fn send_capslock(cfg: &NotifyConfig, enabled: bool) {
    match cfg.backend {
        NotifyBackend::None => {}

        NotifyBackend::SwayOsd => {
            Command::new("swayosd-client").arg("--caps-lock").spawn().ok();
        }

        NotifyBackend::NotifySend => {
            let state = if enabled { "On" } else { "Off" };
            let icon  = if enabled { "caps-lock-symbolic" } else { "caps-lock-off-symbolic" };
            Command::new("notify-send")
                .args([
                    "--icon",        icon,
                    "--expire-time", &cfg.timeout_ms.to_string(),
                    "--urgency",     "low",
                    &format!("Caps Lock {state}"),
                    "",
                ])
                .spawn()
                .ok();
        }
    }
}
