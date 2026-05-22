//! CapsLock monitor.
//!
//! Polls `/sys/class/leds/*/brightness` and fires a notification on change.

use crate::{config::Config, notify::NotifyBackend};
use std::{
    fs,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

fn find_led() -> Option<String> {
    for entry in fs::read_dir("/sys/class/leds").ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().contains("capslock") {
            return Some(format!("/sys/class/leds/{name}/brightness"));
        }
    }
    None
}

fn read_brightness(path: &str) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Spawn the CapsLock monitor thread.
///
/// `notify` is a shared reference to the active backend so it can be swapped
/// on config reload without restarting the thread.
pub fn spawn(cfg: Arc<RwLock<Config>>, notify: Arc<dyn NotifyBackend>) {
    thread::Builder::new()
        .name("capslock".into())
        .spawn(move || run(&cfg, &notify))
        .expect("spawn capslock thread");
}

fn run(cfg: &Arc<RwLock<Config>>, notify: &Arc<dyn NotifyBackend>) {
    let mut led_path:  Option<String> = None;
    let mut prev_brightness: u32      = 0;

    loop {
        let (enabled, poll_ms) = {
            let g = cfg.read().unwrap();
            (g.capslock.enabled, g.capslock.poll_ms)
        };

        if !enabled {
            thread::sleep(Duration::from_secs(1));
            continue;
        }

        if led_path.is_none() {
            match find_led() {
                Some(path) => {
                    eprintln!("[capslock] monitoring {path}");
                    prev_brightness = read_brightness(&path).unwrap_or(0);
                    led_path = Some(path);
                }
                None => {
                    eprintln!("[capslock] LED not found — retrying in 2 s");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            }
        }

        thread::sleep(Duration::from_millis(poll_ms));

        if let Some(path) = &led_path {
            match read_brightness(path) {
                Some(cur) if cur != prev_brightness => {
                    prev_brightness = cur;
                    let on = cur != 0;
                    eprintln!("[capslock] {}", if on { "ON" } else { "off" });
                    notify.capslock_changed(on);
                }
                None => {
                    eprintln!("[capslock] LED disappeared — rescanning");
                    led_path = None;
                }
                _ => {}
            }
        }
    }
}
