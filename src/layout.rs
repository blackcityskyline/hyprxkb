//! Layout state: querying hyprctl, calling hyprctl, OSD.

use crate::config::Config;
use std::{fs, process::Command};

pub fn file_read(cfg: &Config) -> String {
    if let Ok(s) = fs::read_to_string(&cfg.general.layout_file) {
        let s = s.trim().to_owned();
        if !s.is_empty() && cfg.layout_index(&s).is_some() {
            return s;
        }
    }
    // Файла нет — начинаем с первого layout и сохраняем
    let fallback = cfg.keyboard.layouts.first().cloned().unwrap_or_default();
    file_write(cfg, &fallback);
    fallback
}

pub fn file_write(cfg: &Config, layout: &str) {
    if let Err(e) = fs::write(&cfg.general.layout_file, layout) {
        eprintln!("[layout] write state file: {}", e);
    }
}

pub fn hyprctl_set_index(device: &str, idx: usize) {
    match Command::new("hyprctl")
        .args(["switchxkblayout", device, &idx.to_string()])
        .status()
    {
        Ok(s) if !s.success() => eprintln!("[layout] hyprctl exited with {}", s),
        Err(e) => eprintln!("[layout] hyprctl: {}", e),
        _ => {}
    }
}

pub fn osd_notify(cfg: &Config, layout: &str) {
    if !cfg.osd.enabled { return; }
    let msg = cfg.osd_message(layout);
    if let Err(e) = Command::new("swayosd-client")
        .args(["--custom-message", msg, "--custom-icon", &cfg.osd.icon])
        .spawn()
    {
        eprintln!("[osd] swayosd-client: {}", e);
    }
}

#[derive(Debug, Default)]
pub struct State {
    saved_layout: Option<String>,
}

impl State {
    fn apply(cfg: &Config, layout: &str) {
        let current = file_read(cfg);
        if current == layout { return; }
        if let Some(idx) = cfg.layout_index(layout) {
            eprintln!("[layout] {} -> {}", current, layout);
            hyprctl_set_index(&cfg.keyboard.device, idx);
            file_write(cfg, layout);
            osd_notify(cfg, layout);
        } else {
            eprintln!("[layout] unknown layout {:?}", layout);
        }
    }

    #[allow(dead_code)]
    pub fn set(&self, cfg: &Config, layout: &str) {
        Self::apply(cfg, layout);
    }

    pub fn save_and_set_first(&mut self, cfg: &Config) {
        if self.saved_layout.is_none() {
            self.saved_layout = Some(file_read(cfg));
        }
        if let Some(first) = cfg.keyboard.layouts.first().cloned() {
            Self::apply(cfg, &first);
        }
    }

    pub fn restore(&mut self, cfg: &Config) {
        if let Some(saved) = self.saved_layout.take() {
            Self::apply(cfg, &saved);
        }
    }

    pub fn rotate(&mut self, cfg: &Config) {
        let layouts = &cfg.keyboard.layouts;
        if layouts.is_empty() { return; }
        let cur = file_read(cfg);
        let cur_idx = cfg.layout_index(&cur).unwrap_or(0);
        let next_idx = (cur_idx + 1) % layouts.len();
        let next = layouts[next_idx].clone();
        eprintln!("[layout] rotate {} -> {}", cur, next);
        hyprctl_set_index(&cfg.keyboard.device, next_idx);
        file_write(cfg, &next);
        if let Some(slot) = &mut self.saved_layout {
            slot.clone_from(&next);
        }
        osd_notify(cfg, &next);
    }

    pub fn has_saved(&self) -> bool {
        self.saved_layout.is_some()
    }
}
