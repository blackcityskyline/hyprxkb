//! Notification system.
//!
//! Adding a new backend = implement `NotifyBackend` in a new submodule,
//! then add a variant to `create_backend()`.

pub mod none;
pub mod swayosd;
pub mod notify_send;
pub mod quickshell;

use crate::config::{NotifyBackend as BackendKind, NotifyConfig};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// A notification backend.
pub trait NotifyBackend: Send + Sync {
    /// Send a layout-change notification.
    fn layout_changed(&self, label: &str);
    /// Send a CapsLock state notification.
    fn capslock_changed(&self, enabled: bool);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub fn create_backend(cfg: &NotifyConfig) -> Box<dyn NotifyBackend> {
    match cfg.backend {
        BackendKind::None        => Box::new(none::NoneBackend),
        BackendKind::SwayOsd     => Box::new(swayosd::SwayOsdBackend::new(cfg)),
        BackendKind::NotifySend  => Box::new(notify_send::NotifySendBackend::new(cfg)),
        BackendKind::QuickShell  => Box::new(quickshell::QuickShellBackend::new(cfg)),
    }
}

// ---------------------------------------------------------------------------
// Waybar signal helper (shared by all backends)
// ---------------------------------------------------------------------------

/// Send SIGRTMIN+N to all `waybar` processes to trigger an instant bar refresh.
pub fn signal_waybar(sig: u8) {
    // SIGRTMIN = 34 on Linux.
    let signum = 34i32 + sig as i32;
    std::process::Command::new("pkill")
        .args([&format!("-{signum}"), "waybar"])
        .spawn()
        .ok();
}
