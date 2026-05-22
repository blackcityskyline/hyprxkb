//! evdev-based hotkey detection.
//!
//! Reads raw `/dev/input/event*` devices. Works on the lock screen because
//! we bypass the compositor's input grab.

use crate::config::Config;
use evdev::{Device, EventType, KeyCode};
use std::{
    os::unix::io::AsRawFd,
    sync::{mpsc::Sender, Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Key / modifier resolution
// ---------------------------------------------------------------------------

pub fn resolve_key(name: &str) -> Option<KeyCode> {
    match name.to_ascii_lowercase().as_str() {
        "space"  => Some(KeyCode::KEY_SPACE),
        "tab"    => Some(KeyCode::KEY_TAB),
        "enter"  => Some(KeyCode::KEY_ENTER),
        "grave"  => Some(KeyCode::KEY_GRAVE),
        "minus"  => Some(KeyCode::KEY_MINUS),
        "equal"  => Some(KeyCode::KEY_EQUAL),
        "left"   => Some(KeyCode::KEY_LEFT),
        "right"  => Some(KeyCode::KEY_RIGHT),
        "up"     => Some(KeyCode::KEY_UP),
        "down"   => Some(KeyCode::KEY_DOWN),
        "f1"     => Some(KeyCode::KEY_F1),
        "f2"     => Some(KeyCode::KEY_F2),
        "f3"     => Some(KeyCode::KEY_F3),
        "f4"     => Some(KeyCode::KEY_F4),
        "f5"     => Some(KeyCode::KEY_F5),
        "f6"     => Some(KeyCode::KEY_F6),
        "f7"     => Some(KeyCode::KEY_F7),
        "f8"     => Some(KeyCode::KEY_F8),
        "f9"     => Some(KeyCode::KEY_F9),
        "f10"    => Some(KeyCode::KEY_F10),
        "f11"    => Some(KeyCode::KEY_F11),
        "f12"    => Some(KeyCode::KEY_F12),
        _        => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier { Super, Alt, Ctrl, Shift }

impl Modifier {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "super" | "meta" => Some(Self::Super),
            "alt"            => Some(Self::Alt),
            "ctrl"|"control" => Some(Self::Ctrl),
            "shift"          => Some(Self::Shift),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-device modifier state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ModState {
    sup: bool, alt: bool, ctrl: bool, shift: bool,
}

impl ModState {
    fn update(&mut self, key: KeyCode, pressed: bool) {
        match key {
            KeyCode::KEY_LEFTMETA  | KeyCode::KEY_RIGHTMETA  => self.sup   = pressed,
            KeyCode::KEY_LEFTALT   | KeyCode::KEY_RIGHTALT   => self.alt   = pressed,
            KeyCode::KEY_LEFTCTRL  | KeyCode::KEY_RIGHTCTRL  => self.ctrl  = pressed,
            KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => self.shift = pressed,
            _ => {}
        }
    }

    fn held(&self, m: Modifier) -> bool {
        match m {
            Modifier::Super => self.sup,
            Modifier::Alt   => self.alt,
            Modifier::Ctrl  => self.ctrl,
            Modifier::Shift => self.shift,
        }
    }
}

// ---------------------------------------------------------------------------
// Device helpers
// ---------------------------------------------------------------------------

fn is_keyboard(dev: &Device) -> bool {
    dev.supported_keys().map_or(false, |k| {
        k.contains(KeyCode::KEY_SPACE) && k.contains(KeyCode::KEY_LEFTMETA)
    })
}

fn set_nonblocking(fd: i32) {
    unsafe {
        extern "C" { fn fcntl(fd: i32, cmd: i32, ...) -> i32; }
        let flags = fcntl(fd, 3 /* F_GETFL */);
        fcntl(fd, 4 /* F_SETFL */, flags | 2048 /* O_NONBLOCK */);
    }
}

fn find_keyboards() -> Vec<Device> {
    let mut out = Vec::new();
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d)  => d,
        Err(e) => { eprintln!("[evdev] read_dir /dev/input: {e}"); return out; }
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with("event")
        {
            continue;
        }
        match Device::open(&path) {
            Ok(dev) if is_keyboard(&dev) => {
                eprintln!(
                    "[evdev] watching: {} ({})",
                    path.display(),
                    dev.name().unwrap_or("?")
                );
                set_nonblocking(dev.as_raw_fd());
                out.push(dev);
            }
            Ok(_)  => {}
            Err(e) if e.raw_os_error() == Some(libc::EACCES) => {}
            Err(e) => eprintln!("[evdev] open {path:?}: {e}"),
        }
    }
    out
}

fn poll_ready(devices: &[Device], timeout_ms: i32) -> bool {
    let mut pfds: Vec<libc::pollfd> = devices.iter().map(|d| libc::pollfd {
        fd: d.as_raw_fd(), events: libc::POLLIN, revents: 0,
    }).collect();
    let n = unsafe {
        libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, timeout_ms)
    };
    n > 0
}

// ---------------------------------------------------------------------------
// Thread
// ---------------------------------------------------------------------------

/// Events sent to the main loop.
pub enum EvdevEvent {
    Hotkey,
}

/// Spawn the evdev watcher thread. Hotkey fires are sent on `tx`.
pub fn spawn(cfg: Arc<RwLock<Config>>, tx: Sender<EvdevEvent>) {
    thread::Builder::new()
        .name("evdev".into())
        .spawn(move || run(&cfg, &tx))
        .expect("spawn evdev thread");
}

fn read_hotkey(cfg: &RwLock<Config>) -> Option<(KeyCode, Modifier)> {
    let g = cfg.read().unwrap();
    let key = resolve_key(&g.hotkey.key)?;
    let m   = Modifier::parse(&g.hotkey.modifier)?;
    Some((key, m))
}

fn run(cfg: &Arc<RwLock<Config>>, tx: &Sender<EvdevEvent>) {
    const DEBOUNCE: Duration = Duration::from_millis(250);

    let mut hotkey:      Option<(KeyCode, Modifier)> = None;
    let mut devices:     Vec<Device>   = Vec::new();
    let mut mod_states:  Vec<ModState> = Vec::new();
    let mut last_fire:   Instant       = Instant::now()
        .checked_sub(DEBOUNCE * 2).unwrap_or_else(Instant::now);
    let mut last_reload: Instant       = Instant::now()
        .checked_sub(Duration::from_secs(10)).unwrap_or_else(Instant::now);

    loop {
        // Re-read config and re-scan devices every 2 s or when hotkey changes.
        if last_reload.elapsed() >= Duration::from_secs(2) {
            last_reload = Instant::now();
            let new_hk = read_hotkey(cfg);
            if new_hk != hotkey {
                hotkey     = new_hk;
                devices    = find_keyboards();
                mod_states = (0..devices.len()).map(|_| ModState::default()).collect();
                eprintln!("[evdev] hotkey: {hotkey:?}");
            }
        }

        let Some((target_key, modifier)) = hotkey else {
            thread::sleep(Duration::from_secs(1));
            continue;
        };
        if devices.is_empty() {
            thread::sleep(Duration::from_secs(1));
            continue;
        }

        if !poll_ready(&devices, 1000) {
            continue;
        }

        for i in 0..devices.len() {
            let mut rescan = false;
            loop {
                match devices[i].fetch_events() {
                    Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => break,
                    Err(e) => {
                        eprintln!("[evdev] device {i} error: {e}");
                        rescan = true;
                        break;
                    }
                    Ok(events) => {
                        for ev in events {
                            if ev.event_type() != EventType::KEY { continue; }
                            let key   = KeyCode::new(ev.code());
                            let value = ev.value(); // 0=up 1=down 2=repeat
                            mod_states[i].update(key, value != 0);

                            if key == target_key
                                && value == 1
                                && mod_states[i].held(modifier)
                                && last_fire.elapsed() > DEBOUNCE
                            {
                                last_fire = Instant::now();
                                eprintln!("[evdev] hotkey fired");
                                if tx.send(EvdevEvent::Hotkey).is_err() {
                                    return; // main loop gone — exit thread
                                }
                            }
                        }
                    }
                }
            }
            if rescan {
                devices.clear();
                break;
            }
        }
    }
}
