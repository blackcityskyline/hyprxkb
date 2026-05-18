//! evdev thread: discover keyboards, detect hotkey, call `rotate`.

use crate::{config::Config, layout::State};
use evdev::{Device, EventType, InputEventKind, Key};
use std::{
    os::unix::io::AsRawFd,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub fn resolve_key(name: &str) -> Option<Key> {
    match name.to_ascii_lowercase().as_str() {
        "space"  => Some(Key::KEY_SPACE),
        "tab"    => Some(Key::KEY_TAB),
        "enter"  => Some(Key::KEY_ENTER),
        "grave"  => Some(Key::KEY_GRAVE),
        "minus"  => Some(Key::KEY_MINUS),
        "equal"  => Some(Key::KEY_EQUAL),
        "left"   => Some(Key::KEY_LEFT),
        "right"  => Some(Key::KEY_RIGHT),
        "up"     => Some(Key::KEY_UP),
        "down"   => Some(Key::KEY_DOWN),
        "f1"     => Some(Key::KEY_F1),  "f2"  => Some(Key::KEY_F2),
        "f3"     => Some(Key::KEY_F3),  "f4"  => Some(Key::KEY_F4),
        "f5"     => Some(Key::KEY_F5),  "f6"  => Some(Key::KEY_F6),
        "f7"     => Some(Key::KEY_F7),  "f8"  => Some(Key::KEY_F8),
        "f9"     => Some(Key::KEY_F9),  "f10" => Some(Key::KEY_F10),
        "f11"    => Some(Key::KEY_F11), "f12" => Some(Key::KEY_F12),
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

#[derive(Default)]
struct ModState { meta: bool, alt: bool, ctrl: bool, shift: bool }

impl ModState {
    fn update(&mut self, key: Key, pressed: bool) {
        match key {
            Key::KEY_LEFTMETA  | Key::KEY_RIGHTMETA  => self.meta  = pressed,
            Key::KEY_LEFTALT   | Key::KEY_RIGHTALT   => self.alt   = pressed,
            Key::KEY_LEFTCTRL  | Key::KEY_RIGHTCTRL  => self.ctrl  = pressed,
            Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => self.shift = pressed,
            _ => {}
        }
    }
    fn matches(&self, m: Modifier) -> bool {
        match m {
            Modifier::Meta  => self.meta,
            Modifier::Alt   => self.alt,
            Modifier::Ctrl  => self.ctrl,
            Modifier::Shift => self.shift,
        }
    }
}

fn find_keyboards() -> Vec<Device> {
    let mut found = Vec::new();
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d) => d,
        Err(e) => { eprintln!("[evdev] read_dir: {}", e); return found; }
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.file_name().unwrap_or_default().to_string_lossy().starts_with("event") {
            continue;
        }
        match Device::open(&path) {
            Ok(mut dev) => {
                if dev.supported_keys().map_or(false, |k| {
                    k.contains(Key::KEY_SPACE) && k.contains(Key::KEY_LEFTMETA)
                }) {
                    eprintln!("[evdev] watching: {} ({})", path.display(), dev.name().unwrap_or("?"));
                    // O_NONBLOCK через fcntl (evdev 0.12 не имеет set_nonblocking)
                    unsafe { set_nonblocking(dev.as_raw_fd()); }
                    found.push(dev);
                }
            }
            Err(e) if e.raw_os_error() == Some(13) => {} // EACCES — тихо
            Err(e) => eprintln!("[evdev] open {:?}: {}", path, e),
        }
    }
    found
}

pub fn spawn(cfg: Arc<Config>, state: Arc<Mutex<State>>) {
    thread::Builder::new()
        .name("evdev".into())
        .spawn(move || run(&cfg, &state))
        .expect("spawn evdev thread");
}

#[repr(C)]
struct PollFd { fd: i32, events: i16, revents: i16 }

fn poll_wait(fds: &mut Vec<PollFd>, timeout_ms: i32) -> i32 {
    extern "C" { fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32; }
    unsafe { poll(fds.as_mut_ptr(), fds.len() as u64, timeout_ms) }
}

unsafe fn set_nonblocking(fd: i32) {
    extern "C" { fn fcntl(fd: i32, cmd: i32, ...) -> i32; }
    let flags = fcntl(fd, 3 /* F_GETFL */);
    fcntl(fd, 4 /* F_SETFL */, flags | 2048 /* O_NONBLOCK */);
}

fn run(cfg: &Config, state: &Arc<Mutex<State>>) {
    let target_key = match resolve_key(&cfg.hotkey.key) {
        Some(k) => k,
        None => { eprintln!("[evdev] unknown key: {:?}", cfg.hotkey.key); return; }
    };
    let modifier = match Modifier::parse(&cfg.hotkey.modifier) {
        Some(m) => m,
        None => { eprintln!("[evdev] unknown modifier: {:?}", cfg.hotkey.modifier); return; }
    };

    let mut devices = find_keyboards();
    if devices.is_empty() {
        eprintln!("[evdev] no keyboards found");
        return;
    }

    let mut mod_states: Vec<ModState> = (0..devices.len()).map(|_| ModState::default()).collect();
    let debounce = Duration::from_millis(300);
    let mut last_hotkey = Instant::now()
        .checked_sub(debounce + Duration::from_millis(1))
        .unwrap_or_else(Instant::now);

    loop {
        // Блокируемся на poll до появления данных (макс 1 сек)
        let mut pollfds: Vec<PollFd> = devices.iter().map(|d| PollFd {
            fd: d.as_raw_fd(),
            events: 0x0001, // POLLIN
            revents: 0,
        }).collect();

        if poll_wait(&mut pollfds, 1000) < 0 {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        for (i, pfd) in pollfds.iter().enumerate() {
            if pfd.revents == 0 { continue; }

            // Вычитываем ВСЕ накопившиеся события
            loop {
                match devices[i].fetch_events() {
                    Err(e) if e.raw_os_error() == Some(11) => break, // EAGAIN — буфер пуст
                    Err(e) => { eprintln!("[evdev] read error: {}", e); break; }
                    Ok(events) => {
                        for ev in events {
                            if ev.event_type() != EventType::KEY { continue; }
                            let InputEventKind::Key(key) = ev.kind() else { continue };
                            let value = ev.value(); // 0=up 1=down 2=repeat

                            mod_states[i].update(key, value != 0);

                            if key == target_key && value == 1 && mod_states[i].matches(modifier) {
                                let now = Instant::now();
                                if now.duration_since(last_hotkey) > debounce {
                                    last_hotkey = now;
                                    eprintln!("[evdev] hotkey → rotate");
                                    state.lock().unwrap().rotate(cfg);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
