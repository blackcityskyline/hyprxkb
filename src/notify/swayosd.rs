use super::{NotifyBackend, signal_waybar};
use crate::config::NotifyConfig;
use std::process::Command;

pub struct SwayOsdBackend {
    icon:           String,
    waybar_signal:  Option<u8>,
}

impl SwayOsdBackend {
    pub fn new(cfg: &NotifyConfig) -> Self {
        Self {
            icon:          cfg.icon.clone(),
            waybar_signal: cfg.waybar_signal,
        }
    }
}

impl NotifyBackend for SwayOsdBackend {
    fn layout_changed(&self, label: &str) {
        Command::new("swayosd-client")
            .args(["--custom-message", label, "--custom-icon", &self.icon])
            .spawn()
            .ok();
        if let Some(sig) = self.waybar_signal {
            signal_waybar(sig);
        }
    }

    fn capslock_changed(&self, _enabled: bool) {
        Command::new("swayosd-client")
            .arg("--caps-lock")
            .spawn()
            .ok();
    }
}
