//! hypxkb — keyboard layout-switching daemon for Hyprland.
//!
//! Architecture:
//!   • main thread  — reads Hyprland socket2 events, drives auto-switch logic
//!   • evdev thread — reads /dev/input, fires hotkey rotate (works on lockscreen)
//!
//! Shared state (`layout::State`) is protected by a single `Mutex`.

mod config;
mod evdev;
mod hypr;
mod layout;

use config::Config;
use hypr::{EventStream, HyprEvent};
use layout::State;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn main() {
    // ── Config ────────────────────────────────────────────────────────────────

    let cfg_path = match Config::default_path() {
        Some(p) => p,
        None => {
            eprintln!("[main] HOME not set");
            std::process::exit(1);
        }
    };

    Config::write_default(&cfg_path);
    let cfg = Arc::new(Config::load(&cfg_path));

    // ── Shared state ──────────────────────────────────────────────────────────

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));

    // ── evdev thread ──────────────────────────────────────────────────────────

    evdev::spawn(Arc::clone(&cfg), Arc::clone(&state));

    // ── Hyprland socket ───────────────────────────────────────────────────────

    let reader = match hypr::connect() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[main] {}", e);
            std::process::exit(1);
        }
    };

    // Throttle window-focus events: ignore a second event within switch_delay.
    let switch_delay = Duration::from_millis(cfg.general.switch_delay_ms);
    // BUG FIX vs C: `last_switch` is only updated when we *actually act* on
    // an event, not unconditionally on every activewindow line.  This means
    // rapid focus changes still honour the delay, but a focus change that
    // doesn't trigger a layout switch doesn't start the timer.
    let mut last_switch: Option<Instant> = None;

    for event in EventStream::new(reader) {
        match event {
            HyprEvent::ActiveWindow { class, .. } => {
                // Throttle: skip if we switched very recently.
                if let Some(t) = last_switch {
                    if t.elapsed() < switch_delay {
                        continue;
                    }
                }

                let mut st = state.lock().unwrap();

                if cfg.is_english_class(&class) {
                    st.save_and_set_first(&cfg);
                    last_switch = Some(Instant::now());
                } else if st.has_saved() {
                    // Switched away from an English-forced window → restore.
                    st.restore(&cfg);
                    last_switch = Some(Instant::now());
                }
                // else: ordinary app, no active save — do nothing.
            }

            HyprEvent::LayerOpen(name) => {
                if cfg.is_english_layer(&name) {
                    let mut st = state.lock().unwrap();
                    st.save_and_set_first(&cfg);
                    last_switch = Some(Instant::now());
                }
            }

            HyprEvent::LayerClose(name) => {
                if cfg.is_english_layer(&name) {
                    let mut st = state.lock().unwrap();
                    if st.has_saved() {
                        st.restore(&cfg);
                        last_switch = Some(Instant::now());
                    }
                }
            }
        }
    }

    eprintln!("[main] Hyprland socket closed — exiting");
}
