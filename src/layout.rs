//! Layout state: reading/writing the state file, applying layouts via hyprctl.

use crate::{config::Config, notify};
use std::{fs, process::Command};

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

/// Read the persisted layout from disk. Falls back to the first configured layout.
pub fn file_read(cfg: &Config) -> String {
    if let Ok(s) = fs::read_to_string(&cfg.general.layout_file) {
        let s = s.trim().to_owned();
        if !s.is_empty() && cfg.layout_index(&s).is_some() {
            return s;
        }
    }
    let fallback = cfg.keyboard.layouts.first().cloned().unwrap_or_default();
    file_write(cfg, &fallback);
    fallback
}

/// Persist the current layout name to disk.
pub fn file_write(cfg: &Config, layout: &str) {
    if let Err(e) = fs::write(&cfg.general.layout_file, layout) {
        eprintln!("[layout] write state file: {e}");
    }
}

// ---------------------------------------------------------------------------
// hyprctl
// ---------------------------------------------------------------------------

fn hyprctl_set(device: &str, idx: usize) {
    match Command::new("hyprctl")
        .args(["switchxkblayout", device, &idx.to_string()])
        .status()
    {
        Ok(s) if !s.success() => eprintln!("[layout] hyprctl exited with {s}"),
        Err(e)                => eprintln!("[layout] hyprctl: {e}"),
        _                     => {}
    }
}

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// Layout state machine.
///
/// # Invariants
/// - `current` always reflects the layout that was last applied.
/// - `forced_stack` is non-empty only when a forced context is active.
/// - `saved_layout` holds the layout to restore once all forced contexts exit.
#[derive(Debug, Default)]
pub struct State {
    /// The layout currently active (mirrors the state file).
    current:      Option<String>,
    /// Stack of forced layouts (e.g. rofi opened inside a terminal).
    forced_stack: Vec<String>,
    /// Layout to restore when `forced_stack` drains.
    saved_layout: Option<String>,
}

impl State {
    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Apply `layout` via hyprctl and update all local state, but only if it
    /// differs from the currently active layout (avoids redundant syscalls).
    fn apply(&mut self, cfg: &Config, layout: &str) {
        if self.current.as_deref() == Some(layout) {
            return;
        }
        if let Some(idx) = cfg.layout_index(layout) {
            eprintln!("[layout] {} → {layout}", self.current.as_deref().unwrap_or("?"));
            hyprctl_set(&cfg.keyboard.device, idx);
            file_write(cfg, layout);
            self.current = Some(layout.to_owned());
            notify::send(&cfg.notify, cfg.layout_message(layout), "");
        } else {
            eprintln!("[layout] unknown layout {layout:?}");
        }
    }

    /// Ensure `current` is populated (loads from file on first call).
    fn ensure_current(&mut self, cfg: &Config) {
        if self.current.is_none() {
            self.current = Some(file_read(cfg));
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Push a forced layout (e.g. app/layer switched to English-only context).
    pub fn force_push(&mut self, cfg: &Config, layout: &str) {
        self.ensure_current(cfg);
        if self.forced_stack.is_empty() {
            self.saved_layout = self.current.clone();
        }
        self.forced_stack.push(layout.to_owned());
        self.apply(cfg, layout);
    }

    /// Pop the topmost forced layout and restore the previous one.
    pub fn force_pop(&mut self, cfg: &Config) {
        if self.forced_stack.is_empty() {
            eprintln!("[layout] force_pop called with empty stack — ignoring");
            return;
        }
        self.forced_stack.pop();
        let next = self.forced_stack.last().cloned()
            .or_else(|| self.saved_layout.take());
        if let Some(layout) = next {
            self.apply(cfg, &layout);
        }
    }

    /// Whether a forced context is currently active.
    pub fn is_forced(&self) -> bool {
        !self.forced_stack.is_empty()
    }

    /// Rotate to the next layout in the configured list (global hotkey action).
    pub fn rotate(&mut self, cfg: &Config) {
        self.ensure_current(cfg);
        self.forced_stack.clear();
        self.saved_layout = None;

        let layouts = &cfg.keyboard.layouts;
        if layouts.is_empty() { return; }

        let cur = self.current.as_deref().unwrap_or("");
        let cur_idx = cfg.layout_index(cur).unwrap_or(0);
        let next_idx = (cur_idx + 1) % layouts.len();
        let next = layouts[next_idx].clone();
        eprintln!("[layout] rotate: {cur} → {next}");
        self.apply(cfg, &next);
    }

    /// Force-set a specific layout by name (used by the CLI `switch` command).
    pub fn switch_to(&mut self, cfg: &Config, layout: &str) {
        self.forced_stack.clear();
        self.saved_layout = None;
        self.apply(cfg, layout);
    }

    /// Return the name of the currently active layout, if known.
    #[allow(dead_code)]
    pub fn current_layout(&self) -> Option<&str> {
        self.current.as_deref()
    }
}
