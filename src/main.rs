//! hyprxkb — keyboard layout-switching utility for Hyprland.
//!
//! # Usage
//!
//! ```
//! hyprxkb                    # start (background process)
//! hyprxkb init               # write default config and exit
//! hyprxkb reload             # reload config (sends SIGUSR1 to running instance)
//! hyprxkb status             # print current layout label (plain text)
//! hyprxkb status --json      # print JSON for waybar
//! hyprxkb switch <layout>    # switch to a specific XKB layout
//! hyprxkb list               # list all configured layouts
//! ```

mod capslock;
mod compositor;
mod config;
mod input;
mod instance;
mod layout;
mod notify;

use compositor::{hyprland::HyprlandCompositor, Compositor, CompositorEvent};
use config::Config;
use input::evdev::{self, EvdevEvent};
use layout::{Action, Engine, EngineInput};
use notify::NotifyBackend;
use signal_hook::{consts::SIGUSR1, iterator::Signals};
use std::{
    env, fs, io, process,
    sync::{mpsc, Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("init")   => cmd_init(),
        Some("reload") => cmd_reload(),
        Some("status") => cmd_status(args.get(2).map(String::as_str) == Some("--json")),
        Some("switch") => cmd_switch(args.get(2).map(String::as_str)),
        Some("list")   => cmd_list(),
        Some("--version") | Some("-V") => {
            println!("hyprxkb {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }
        Some(unknown) => {
            eprintln!("hyprxkb: unknown command {unknown:?}");
            eprintln!(
                "Usage: hyprxkb \
                 [init | reload | status [--json] | switch <layout> | list | --version]"
            );
            process::exit(1);
        }
        None => run(),
    }
}

// ---------------------------------------------------------------------------
// CLI commands
// ---------------------------------------------------------------------------

fn cfg_path() -> std::path::PathBuf {
    Config::default_path().unwrap_or_else(|| {
        eprintln!("[main] HOME is not set");
        process::exit(1);
    })
}

/// `hyprxkb init` — write the default config skeleton.
fn cmd_init() {
    let path = cfg_path();
    if Config::write_default(&path) {
        println!("Created: {}", path.display());
        println!("Edit it, then run: hyprxkb");
    } else {
        println!("Config already exists: {}", path.display());
        println!("Delete it first if you want to regenerate the skeleton.");
    }
    process::exit(0);
}

/// `hyprxkb reload` — send SIGUSR1 to the running instance.
fn cmd_reload() {
    let pid = instance::InstanceGuard::running_pid().unwrap_or_else(|| {
        eprintln!("[reload] hyprxkb is not running");
        process::exit(1);
    });
    let ret = unsafe { libc::kill(pid as libc::pid_t, libc::SIGUSR1) };
    if ret != 0 {
        eprintln!(
            "[reload] kill({pid}, SIGUSR1): {}",
            io::Error::last_os_error()
        );
        process::exit(1);
    }
    println!("Config reload requested (PID {pid}).");
    process::exit(0);
}

/// `hyprxkb status [--json]`
fn cmd_status(json: bool) {
    let cfg    = Config::load(&cfg_path());
    let layout = state_file_read(&cfg);
    let label  = cfg.label(&layout).to_owned();
    if json {
        println!(
            r#"{{"text":{},"layout":{},"tooltip":"hyprxkb"}}"#,
            json_str(&label),
            json_str(&layout),
        );
    } else {
        println!("{label}");
    }
    process::exit(0);
}

/// `hyprxkb switch <layout>`
fn cmd_switch(layout: Option<&str>) {
    let layout = layout.unwrap_or_else(|| {
        eprintln!("Usage: hyprxkb switch <layout>");
        process::exit(1);
    });
    let cfg = Config::load(&cfg_path());
    if let Some(idx) = cfg.layout_index(layout) {
        hyprctl_set(&cfg.keyboard.device, idx);
        state_file_write(&cfg, layout);
        println!("Switched to {layout} ({})", cfg.label(layout));
    } else {
        eprintln!(
            "Unknown layout {layout:?}. Configured: {:?}",
            cfg.keyboard.layouts
        );
        process::exit(1);
    }
    process::exit(0);
}

/// `hyprxkb list`
fn cmd_list() {
    let cfg     = Config::load(&cfg_path());
    let current = state_file_read(&cfg);
    for l in &cfg.keyboard.layouts {
        let marker = if l == &current { "▶" } else { " " };
        println!("{marker}  {l:<8}  {}", cfg.label(l));
    }
    process::exit(0);
}

// ---------------------------------------------------------------------------
// Unified event type
// ---------------------------------------------------------------------------

enum AppEvent {
    Compositor(CompositorEvent),
    Hotkey,
    /// Periodic tick to query compositor for external layout changes.
    SyncTick,
    ConfigReloaded,
}

// ---------------------------------------------------------------------------
// Main run loop
// ---------------------------------------------------------------------------

fn run() {
    // Single-instance guard.
    let _guard = instance::InstanceGuard::acquire().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        process::exit(1);
    });

    let path = cfg_path();
    Config::write_default(&path);
    let cfg = Arc::new(RwLock::new(Config::load(&path)));

    // Notify backend — shared with capslock thread.
    let notify: Arc<dyn NotifyBackend> = {
        let g = cfg.read().unwrap();
        Arc::from(notify::create_backend(&g.notify))
    };

    // State engine.
    let mut engine = Engine::default();
    {
        let g   = cfg.read().unwrap();
        let cur = state_file_read(&g);
        engine.init(cur);
    }

    // Unified event channel.
    let (tx, rx) = mpsc::channel::<AppEvent>();

    // ── evdev thread ──────────────────────────────────────────────────────
    // Bridge EvdevEvent → AppEvent.
    {
        let (etx, erx) = mpsc::channel::<EvdevEvent>();
        let tx2 = tx.clone();
        thread::Builder::new()
            .name("evdev-bridge".into())
            .spawn(move || {
                for ev in erx {
                    let app_ev = match ev { EvdevEvent::Hotkey => AppEvent::Hotkey };
                    if tx2.send(app_ev).is_err() { break; }
                }
            })
            .expect("spawn evdev-bridge thread");
        evdev::spawn(Arc::clone(&cfg), etx);
    }

    // ── CapsLock thread ───────────────────────────────────────────────────
    capslock::spawn(Arc::clone(&cfg), Arc::clone(&notify));

    // ── Signal thread (SIGUSR1 = reload) ──────────────────────────────────
    {
        let tx2   = tx.clone();
        let cfg2  = Arc::clone(&cfg);
        let path2 = path.clone();
        thread::Builder::new()
            .name("signals".into())
            .spawn(move || {
                let mut sigs = Signals::new([SIGUSR1]).expect("register SIGUSR1");
                for sig in sigs.forever() {
                    if sig == SIGUSR1 {
                        eprintln!("[main] SIGUSR1 — reloading config");
                        let new_cfg = Config::load(&path2);
                        *cfg2.write().unwrap() = new_cfg;
                        eprintln!("[main] config reloaded");
                        let _ = tx2.send(AppEvent::ConfigReloaded);
                    }
                }
            })
            .expect("spawn signal thread");
    }

    // ── Compositor thread ─────────────────────────────────────────────────
    {
        let tx2 = tx.clone();
        thread::Builder::new()
            .name("compositor".into())
            .spawn(move || {
                let mut c = HyprlandCompositor::connect().unwrap_or_else(|e| {
                    eprintln!("[main] {e}");
                    process::exit(1);
                });
                loop {
                    match c.next_event() {
                        Some(ev) => {
                            if tx2.send(AppEvent::Compositor(ev)).is_err() { break; }
                        }
                        None => {
                            eprintln!("[main] compositor disconnected — exiting");
                            process::exit(0);
                        }
                    }
                }
            })
            .expect("spawn compositor thread");
    }

    // ── Periodic sync thread ──────────────────────────────────────────────
    // Queries `hyprctl activelayout` to detect external layout changes.
    {
        let tx2  = tx.clone();
        let cfg2 = Arc::clone(&cfg);
        thread::Builder::new()
            .name("sync-timer".into())
            .spawn(move || {
                loop {
                    let interval = Duration::from_millis(
                        cfg2.read().unwrap().general.sync_interval_ms
                    );
                    thread::sleep(interval);
                    if tx2.send(AppEvent::SyncTick).is_err() { break; }
                }
            })
            .expect("spawn sync-timer thread");
    }

    // ── Event processing loop ─────────────────────────────────────────────
    let mut last_auto_switch = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);

    for app_event in &rx {
        let input: Option<EngineInput> = match app_event {
            AppEvent::Compositor(ev) => {
                compositor_to_engine(ev, &mut last_auto_switch, &cfg)
            }
            AppEvent::Hotkey => Some(EngineInput::Hotkey),
            AppEvent::SyncTick => {
                // Fire a quick hyprctl query to detect external changes.
                query_active_layout(&cfg)
                    .map(|layout| EngineInput::ExternalSync { layout })
            }
            AppEvent::ConfigReloaded => None,
        };

        let Some(input) = input else { continue };

        let actions = {
            let g = cfg.read().unwrap();
            engine.process(&g, input)
        };

        if !actions.is_empty() {
            execute_actions(&actions, &cfg, &*notify);
            engine.commit(&actions);
        }
    }
}

// ---------------------------------------------------------------------------
// Event → EngineInput conversion
// ---------------------------------------------------------------------------

fn compositor_to_engine(
    ev:               CompositorEvent,
    last_auto_switch: &mut Instant,
    cfg:              &Arc<RwLock<Config>>,
) -> Option<EngineInput> {
    match ev {
        CompositorEvent::WindowFocus { class } => {
            let delay = Duration::from_millis(
                cfg.read().unwrap().general.switch_delay_ms
            );
            if last_auto_switch.elapsed() < delay {
                return None;
            }
            *last_auto_switch = Instant::now();
            Some(EngineInput::WindowFocus { class })
        }
        CompositorEvent::LayerOpen { name }  => Some(EngineInput::LayerOpen { name }),
        CompositorEvent::LayerClose { name } => Some(EngineInput::LayerClose { name }),
    }
}

/// Query the compositor for the currently active layout name.
/// Returns `None` if the query fails or the layout is unknown.
fn query_active_layout(cfg: &Arc<RwLock<Config>>) -> Option<String> {
    let device = cfg.read().unwrap().keyboard.device.clone();
    let layouts = cfg.read().unwrap().keyboard.layouts.clone();

    // `hyprctl -j activelayout` → JSON array of {device, layout (display name)}.
    // We can't reliably map display name → XKB id without a full table,
    // so instead we use `hyprctl -j getoption input:kb_layout` which returns
    // the comma-separated raw layout list, and compare against our list using
    // the current active index from `activelayout`.
    //
    // Simpler approach: `hyprctl -j devices` lists keyboards with activeKeymap.
    // We match by device name and map position back to our layouts array.
    let out = std::process::Command::new("hyprctl")
        .args(["-j", "devices"])
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let text = std::str::from_utf8(&out.stdout).ok()?;

    // Scan for our device and extract "activeKeymap".
    // JSON structure: {"keyboards": [{"devnode":"...", "name":"...", "activeKeymap":"...", ...}]}
    // We look for a block containing our device name, then extract activeKeymap.
    let device_lower = device.to_ascii_lowercase();
    for block in text.split('{') {
        let block_lower = block.to_ascii_lowercase();
        if !block_lower.contains(&device_lower) { continue; }
        if let Some(pos) = block.find("\"activeKeymap\"") {
            let rest = &block[pos + 14..];
            if let Some(start) = rest.find('"') {
                let inner = &rest[start + 1..];
                if let Some(end) = inner.find('"') {
                    let keymap = inner[..end].to_owned();
                    // Map display keymap name to XKB id by position.
                    // Hyprland lists keymaps in the same order as keyboard.layouts.
                    // We get the index from `hyprctl activelayout`.
                    // For now: try to match by prefix or substring.
                    for l in &layouts {
                        if keymap.to_ascii_lowercase().contains(&l.to_ascii_lowercase()) {
                            return Some(l.clone());
                        }
                    }
                    eprintln!("[sync] cannot map keymap {keymap:?} to a known layout — skipping");
                    return None;
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Action executor
// ---------------------------------------------------------------------------

fn execute_actions(
    actions: &[Action],
    cfg:     &Arc<RwLock<Config>>,
    notify:  &dyn NotifyBackend,
) {
    // Collect everything we need before taking any locks in a hot path.
    for action in actions {
        match action {
            Action::ApplyLayout { layout, index } => {
                let device = cfg.read().unwrap().keyboard.device.clone();
                hyprctl_set(&device, *index);
                eprintln!("[runner] layout → {layout} (index {index})");
            }
            Action::PersistLayout { layout } => {
                let g = cfg.read().unwrap();
                state_file_write(&g, layout);
            }
            Action::Notify { layout } => {
                let label = cfg.read().unwrap().label(layout).to_owned();
                notify.layout_changed(&label);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// hyprctl helper
// ---------------------------------------------------------------------------

fn hyprctl_set(device: &str, index: usize) {
    // "all" is a special alias — switch every keyboard Hyprland knows about.
    // This is useful when multiple devices share the same layout list and the
    // user wants them kept in sync (e.g. physical keyboard + virtual keyd device).
    let targets: Vec<String> = if device == "all" {
        hyprctl_list_keyboards()
    } else {
        vec![device.to_owned()]
    };

    for target in &targets {
        match std::process::Command::new("hyprctl")
            .args(["switchxkblayout", target, &index.to_string()])
            .status()
        {
            Ok(s) if !s.success() => eprintln!("[runner] hyprctl switchxkblayout {target}: exited {s}"),
            Err(e)                 => eprintln!("[runner] hyprctl: {e}"),
            _                      => {}
        }
    }
}

/// Return the names of all keyboards Hyprland currently knows about.
/// Parses `hyprctl -j devices` — used when `device = "all"`.
fn hyprctl_list_keyboards() -> Vec<String> {
    let out = match std::process::Command::new("hyprctl")
        .args(["-j", "devices"])
        .output()
    {
        Ok(o)  => o,
        Err(e) => { eprintln!("[runner] hyprctl -j devices: {e}"); return vec![]; }
    };
    let text = match std::str::from_utf8(&out.stdout) {
        Ok(t)  => t,
        Err(_) => return vec![],
    };
    // Extract "name":"VALUE" pairs from the JSON.
    // We look for the literal sequence: "name":"<value>"
    // and collect all values — hyprctl lists keyboards, mice, tablets, etc.
    // All of them accept switchxkblayout; Hyprland silently ignores non-keyboards.
    let needle = "\"name\":\"";
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(needle) {
        rest = &rest[pos + needle.len()..];
        if let Some(end) = rest.find('"') {
            let name = rest[..end].to_owned();
            rest = &rest[end + 1..];
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// State file helpers
// ---------------------------------------------------------------------------

fn state_file_read(cfg: &Config) -> String {
    if let Ok(s) = fs::read_to_string(&cfg.general.state_file) {
        let s = s.trim().to_owned();
        if !s.is_empty() && cfg.layout_index(&s).is_some() {
            return s;
        }
    }
    let fallback = cfg.keyboard.layouts.first().cloned().unwrap_or_default();
    state_file_write(cfg, &fallback);
    fallback
}

fn state_file_write(cfg: &Config, layout: &str) {
    if let Err(e) = fs::write(&cfg.general.state_file, layout) {
        eprintln!("[runner] cannot write state file: {e}");
    }
}

// ---------------------------------------------------------------------------
// JSON mini-helper
// ---------------------------------------------------------------------------

fn json_str(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
