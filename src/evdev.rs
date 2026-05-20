//! evdev thread: watches raw keyboard input for the configured hotkey and
//! calls `State::rotate` when it fires.
//!
//! Works even on the lock screen because we read `/dev/input` directly.

use crate::{config::Config, layout::State};
use evdev::{Device, EventType, KeyCode};
use std::{
    os::unix::io::AsRawFd,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Key / modifier resolution
// ---------------------------------------------------------------------------

pub fn resolve_key(name: &str) -> Option<KeyCode> {
    match name.to_ascii_lowercase().as_str() {
        "space" => Some(KeyCode::KEY_SPACE),
        "tab"   => Some(KeyCode::KEY_TAB),
        "enter" => Some(KeyCode::KEY_ENTER),
        "grave" => Some(KeyCode::KEY_GRAVE),
        "minus" => Some(KeyCode::KEY_MINUS),
        "equal" => Some(KeyCode::KEY_EQUAL),
        "left"  => Some(KeyCode::KEY_LEFT),
        "right" => Some(KeyCode::KEY_RIGHT),
        "up"    => Some(KeyCode::KEY_UP),
        "down"  => Some(KeyCode::KEY_DOWN),
        "f1"    => Some(KeyCode::KEY_F1),  "f2"  => Some(KeyCode::KEY_F2),
        "f3"    => Some(KeyCode::KEY_F3),  "f4"  => Some(KeyCode::KEY_F4),
        "f5"    => Some(KeyCode::KEY_F5),  "f6"  => Some(KeyCode::KEY_F6),
        "f7"    => Some(KeyCode::KEY_F7),  "f8"  => Some(KeyCode::KEY_F8),
        "f9"    => Some(KeyCode::KEY_F9),  "f10" => Some(KeyCode::KEY_F10),
        "f11"   => Some(KeyCode::KEY_F11), "f12" => Some(KeyCode::KEY_F12),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier { Meta, Alt, Ctrl, Shift }

impl Modifier {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "meta" | "super"   => Some(Self::Meta),
            "alt"              => Some(Self::Alt),
            "ctrl" | "control" => Some(Self::Ctrl),
            "shift"            => Some(Self::Shift),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-device modifier tracking
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ModState { meta: bool, alt: bool, ctrl: bool, shift: bool }

impl ModState {
    fn update(&mut self, key: KeyCode, pressed: bool) {
        match key {
            KeyCode::KEY_LEFTMETA  | KeyCode::KEY_RIGHTMETA  => self.meta  = pressed,
            KeyCode::KEY_LEFTALT   | KeyCode::KEY_RIGHTALT   => self.alt   = pressed,
            KeyCode::KEY_LEFTCTRL  | KeyCode::KEY_RIGHTCTRL  => self.ctrl  = pressed,
            KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => self.shift = pressed,
            _ => {}
        }
    }

    fn is_held(&self, m: Modifier) -> bool {
        match m {
            Modifier::Meta  => self.meta,
            Modifier::Alt   => self.alt,
            Modifier::Ctrl  => self.ctrl,
            Modifier::Shift => self.shift,
        }
    }
}

// ---------------------------------------------------------------------------
// Device discovery
// ---------------------------------------------------------------------------

fn is_keyboard(dev: &Device) -> bool {
    dev.supported_keys().map_or(false, |keys| {
        keys.contains(KeyCode::KEY_SPACE) && keys.contains(KeyCode::KEY_LEFTMETA)
    })
}

fn set_nonblocking(fd: i32) {
    // SAFETY: standard fcntl usage on a valid file descriptor.
    unsafe {
        extern "C" { fn fcntl(fd: i32, cmd: i32, ...) -> i32; }
        let flags = fcntl(fd, /* F_GETFL */ 3);
        fcntl(fd, /* F_SETFL */ 4, flags | /* O_NONBLOCK */ 2048);
    }
}

fn find_keyboards() -> Vec<Device> {
    let mut keyboards = Vec::new();
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d)  => d,
        Err(e) => { eprintln!("[evdev] read_dir /dev/input: {e}"); return keyboards; }
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.file_name().unwrap_or_default().to_string_lossy().starts_with("event") {
            continue;
        }
        match Device::open(&path) {
            Ok(dev) if is_keyboard(&dev) => {
                eprintln!("[evdev] watching: {} ({})", path.display(), dev.name().unwrap_or("?"));
                set_nonblocking(dev.as_raw_fd());
                keyboards.push(dev);
            }
            Ok(_)  => {}
            Err(e) if e.raw_os_error() == Some(libc::EACCES) => {} // no permission — skip silently
            Err(e) => eprintln!("[evdev] open {path:?}: {e}"),
        }
    }
    keyboards
}

// ---------------------------------------------------------------------------
// poll(2) wrapper
// ---------------------------------------------------------------------------

fn wait_for_input(devices: &[Device], timeout_ms: i32) -> bool {
    let mut pollfds: Vec<libc::pollfd> = devices.iter().map(|d| libc::pollfd {
        fd:      d.as_raw_fd(),
        events:  libc::POLLIN,
        revents: 0,
    }).collect();
    // SAFETY: pollfds is a valid slice of libc::pollfd.
    let n = unsafe {
        libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, timeout_ms)
    };
    n > 0
}

// ---------------------------------------------------------------------------
// Thread entry point
// ---------------------------------------------------------------------------

pub fn spawn(cfg: Arc<RwLock<Config>>, state: Arc<RwLock<State>>) {
    thread::Builder::new()
        .name("evdev".into())
        .spawn(move || run(&cfg, &state))
        .expect("spawn evdev thread");
}

/// Read the current hotkey config. Returns `None` if the config is invalid.
fn read_hotkey(cfg: &RwLock<Config>) -> Option<(KeyCode, Modifier)> {
    let g = cfg.read().unwrap();
    let key = resolve_key(&g.hotkey.key)?;
    let m   = Modifier::parse(&g.hotkey.modifier)?;
    Some((key, m))
}

fn run(cfg: &Arc<RwLock<Config>>, state: &Arc<RwLock<State>>) {
    let debounce = Duration::from_millis(300);

    let mut hotkey:         Option<(KeyCode, Modifier)> = None;
    let mut devices:        Vec<Device>   = Vec::new();
    let mut mod_states:     Vec<ModState> = Vec::new();
    let mut last_hotkey:    Instant       = Instant::now();
    let mut last_cfg_check: Instant       = Instant::now();

    // Reload hotkey config and re-scan keyboards every 2 s.
    let refresh = |hotkey: &mut Option<(KeyCode, Modifier)>,
                       devices: &mut Vec<Device>,
                       mod_states: &mut Vec<ModState>| {
        let new_hotkey = read_hotkey(cfg);
        if *hotkey != new_hotkey {
            *hotkey = new_hotkey;
            *devices = find_keyboards();
            *mod_states = (0..devices.len()).map(|_| ModState::default()).collect();
            eprintln!("[evdev] hotkey: {hotkey:?}");
        }
    };

    refresh(&mut hotkey, &mut devices, &mut mod_states);

    loop {
        // Periodic config refresh.
        if last_cfg_check.elapsed() >= Duration::from_secs(2) {
            last_cfg_check = Instant::now();
            refresh(&mut hotkey, &mut devices, &mut mod_states);
        }

        // If configuration is invalid or no keyboards found, back off.
        let Some((target_key, modifier)) = hotkey else {
            thread::sleep(Duration::from_secs(1));
            continue;
        };
        if devices.is_empty() {
            thread::sleep(Duration::from_secs(1));
            continue;
        }

        if !wait_for_input(&devices, 1000) {
            continue;
        }

        for i in 0..devices.len() {
            loop {
                match devices[i].fetch_events() {
                    Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => break,
                    Err(e) => { eprintln!("[evdev] read error on device {i}: {e}"); break; }
                    Ok(events) => {
                        for ev in events {
                            if ev.event_type() != EventType::KEY { continue; }
                            let key   = KeyCode::new(ev.code());
                            let value = ev.value(); // 0 = up, 1 = down, 2 = repeat

                            mod_states[i].update(key, value != 0);

                            let hotkey_fired = key == target_key
                                && value == 1
                                && mod_states[i].is_held(modifier);

                            if hotkey_fired && last_hotkey.elapsed() > debounce {
                                last_hotkey = Instant::now();
                                eprintln!("[evdev] hotkey → rotate");
                                state.write().unwrap().rotate(&cfg.read().unwrap());
                            }
                        }
                    }
                }
            }
        }
    }
}
