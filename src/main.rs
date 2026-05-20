//! hyprxkb — keyboard layout-switching daemon for Hyprland.
//!
//! Usage:
//!   hyprxkb              — start the daemon
//!   hyprxkb reload       — send SIGUSR1 to the running daemon (reload config)
//!   hyprxkb switch <lay> — switch to a specific layout
//!   hyprxkb status       — print current layout to stdout

mod capslock;
mod config;
mod evdev;
mod hypr;
mod layout;
mod notify;

use config::Config;
use hypr::{EventStream, HyprEvent};
use layout::State;
use signal_hook::{consts::SIGUSR1, iterator::Signals};
use std::{
    env, fs, process,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

const PID_FILE: &str = "/tmp/hyprxkb.pid";

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("reload")           => cmd_reload(),
        Some("status")           => cmd_status(),
        Some("switch")           => cmd_switch(args.get(2).map(String::as_str)),
        Some(unknown)            => {
            eprintln!("Unknown command: {unknown}");
            eprintln!("Usage: hyprxkb [reload | status | switch <layout>]");
            process::exit(1);
        }
        None                     => daemon(),
    }
}

// ---------------------------------------------------------------------------
// CLI commands (send signals / read state file)
// ---------------------------------------------------------------------------

fn read_pid() -> i32 {
    match fs::read_to_string(PID_FILE).ok().and_then(|s| s.trim().parse().ok()) {
        Some(pid) => pid,
        None => {
            eprintln!("[cmd] cannot read PID from {PID_FILE} — is hyprxkb running?");
            process::exit(1);
        }
    }
}

fn cmd_reload() {
    let pid = read_pid();
    // SAFETY: kill is safe with a valid PID and signal number.
    unsafe { libc::kill(pid, libc::SIGUSR1); }
    eprintln!("[reload] sent SIGUSR1 to PID {pid}");
}

fn cmd_status() {
    let cfg_path = cfg_path_or_exit();
    let cfg = Config::load(&cfg_path);
    let layout = layout::file_read(&cfg);
    println!("{}", cfg.layout_message(&layout));
}

fn cmd_switch(layout: Option<&str>) {
    let layout = layout.unwrap_or_else(|| {
        eprintln!("Usage: hyprxkb switch <layout>");
        process::exit(1);
    });
    let cfg_path = cfg_path_or_exit();
    let cfg = Config::load(&cfg_path);
    let mut state = State::default();
    state.switch_to(&cfg, layout);
}

// ---------------------------------------------------------------------------
// Daemon
// ---------------------------------------------------------------------------

fn cfg_path_or_exit() -> std::path::PathBuf {
    Config::default_path().unwrap_or_else(|| {
        eprintln!("[main] HOME not set");
        process::exit(1);
    })
}

fn daemon() {
    // Write PID file so `hyprxkb reload` can find us.
    if let Err(e) = fs::write(PID_FILE, process::id().to_string()) {
        eprintln!("[main] cannot write PID file: {e}");
    }

    let cfg_path = cfg_path_or_exit();
    Config::write_default(&cfg_path);

    let cfg   = Arc::new(RwLock::new(Config::load(&cfg_path)));
    let state = Arc::new(RwLock::new(State::default()));

    // SIGUSR1 → reload config.
    {
        let cfg = Arc::clone(&cfg);
        let path = cfg_path.clone();
        thread::spawn(move || {
            let mut signals = Signals::new([SIGUSR1]).expect("register signal handler");
            for _ in signals.forever() {
                eprintln!("[main] reloading config…");
                *cfg.write().unwrap() = Config::load(&path);
                eprintln!("[main] config reloaded");
            }
        });
    }

    evdev::spawn(Arc::clone(&cfg), Arc::clone(&state));
    capslock::spawn(Arc::clone(&cfg));

    let reader = hypr::connect().unwrap_or_else(|e| {
        eprintln!("[main] {e}");
        process::exit(1);
    });

    run_event_loop(reader, &cfg, &state);
    eprintln!("[main] Hyprland socket closed — exiting");
}

// ---------------------------------------------------------------------------
// Hyprland event loop
// ---------------------------------------------------------------------------

fn run_event_loop(
    reader: impl std::io::BufRead,
    cfg:   &Arc<RwLock<Config>>,
    state: &Arc<RwLock<State>>,
) {
    let mut last_switch = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);

    for event in EventStream::new(reader) {
        let switch_delay = Duration::from_millis(cfg.read().unwrap().general.switch_delay_ms);
        if last_switch.elapsed() < switch_delay {
            continue;
        }

        let switched = handle_event(event, cfg, state);
        if switched {
            last_switch = Instant::now();
        }
    }
}

/// Handle one Hyprland event. Returns `true` if a layout switch was performed.
fn handle_event(
    event: HyprEvent,
    cfg:   &Arc<RwLock<Config>>,
    state: &Arc<RwLock<State>>,
) -> bool {
    let cfg_guard = cfg.read().unwrap();
    let mut st    = state.write().unwrap();

    match event {
        HyprEvent::ActiveWindow { class, .. } => {
            if let Some(layout) = cfg_guard.layout_for_class(&class) {
                st.force_push(&cfg_guard, layout);
                true
            } else if st.is_forced() {
                st.force_pop(&cfg_guard);
                true
            } else {
                false
            }
        }

        HyprEvent::LayerOpen(name) => {
            if let Some(layout) = cfg_guard.layout_for_layer(&name) {
                st.force_push(&cfg_guard, layout);
                true
            } else {
                false
            }
        }

        HyprEvent::LayerClose(name) => {
            if cfg_guard.layout_for_layer(&name).is_some() && st.is_forced() {
                st.force_pop(&cfg_guard);
                true
            } else {
                false
            }
        }
    }
}
