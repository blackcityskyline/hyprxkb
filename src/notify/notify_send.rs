use super::{NotifyBackend, signal_waybar};
use crate::config::NotifyConfig;
use std::process::Command;

pub struct NotifySendBackend {
    icon:          String,
    timeout_ms:    u64,
    waybar_signal: Option<u8>,
}

impl NotifySendBackend {
    pub fn new(cfg: &NotifyConfig) -> Self {
        Self {
            icon:          cfg.icon.clone(),
            timeout_ms:    cfg.timeout_ms,
            waybar_signal: cfg.waybar_signal,
        }
    }
}

impl NotifyBackend for NotifySendBackend {
    fn layout_changed(&self, label: &str) {
        Command::new("notify-send")
            .args([
                "--icon",        &self.icon,
                "--expire-time", &self.timeout_ms.to_string(),
                "--urgency",     "low",
                label,
                "",
            ])
            .spawn()
            .ok();
        if let Some(sig) = self.waybar_signal {
            signal_waybar(sig);
        }
    }

    fn capslock_changed(&self, enabled: bool) {
        let state = if enabled { "On" } else { "Off" };
        let icon  = if enabled { "caps-lock-symbolic" } else { "caps-lock-off-symbolic" };
        Command::new("notify-send")
            .args([
                "--icon",        icon,
                "--expire-time", &self.timeout_ms.to_string(),
                "--urgency",     "low",
                &format!("Caps Lock {state}"),
                "",
            ])
            .spawn()
            .ok();
    }
}
