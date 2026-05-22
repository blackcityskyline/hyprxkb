//! Layout state machine.
//!
//! `Engine` is a pure state machine: it receives `EngineInput` values and
//! returns `Vec<Action>`. All I/O (hyprctl, file writes, notifications) is
//! performed by the caller (`Runner` in `main.rs`).
//!
//! # State invariants
//! - `current` — the layout we last *told the compositor* to apply.
//!   May diverge from reality if an external tool changed it; re-synced via
//!   `EngineInput::ExternalSync`.
//! - `forced_stack` — stack of `(source_id, layout)` pairs. `source_id` is an
//!   opaque string (layer name or window class) so we can pop the exact frame
//!   that was pushed, even if multiple force contexts are nested.
//! - `saved` — the "free" layout to restore once the forced stack drains.
//! - `memory` — per-window layout memory map (app_class → layout).

pub mod rules;

use crate::config::Config;
use rules::RuleMatch;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public input / output types
// ---------------------------------------------------------------------------

/// Input to the engine. One value per compositor/hotkey/sync event.
#[derive(Debug, Clone)]
pub enum EngineInput {
    /// A window received focus.
    WindowFocus { class: String },
    /// A layer surface was opened.
    LayerOpen { name: String },
    /// A layer surface was closed.
    LayerClose { name: String },
    /// User pressed the hotkey — rotate to the next layout.
    Hotkey,
    /// Explicit switch to a named layout (CLI `switch` command).
    #[allow(dead_code)]
    SwitchTo { layout: String },
    /// Periodic sync: the compositor's real active layout (may differ from ours).
    ExternalSync { layout: String },
}

/// Actions the runner must perform after processing an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Tell the compositor to activate this layout index.
    ApplyLayout {
        /// XKB layout name (e.g. "us", "ru").
        layout: String,
        /// Pre-resolved index into `keyboard.layouts`.
        index: usize,
    },
    /// Persist the layout name to the state file.
    PersistLayout { layout: String },
    /// Send a layout-change notification.
    Notify { layout: String },
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Engine {
    /// Last layout we issued an ApplyLayout for.
    current: Option<String>,
    /// Stack of (source_id, forced_layout).
    forced_stack: Vec<(String, String)>,
    /// Layout to restore when forced_stack drains.
    saved: Option<String>,
    /// Per-window memory: class → layout.
    memory: HashMap<String, String>,
    /// Class of the currently focused window (for memory recording).
    focused_class: Option<String>,
}

impl Engine {
    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Build the ApplyLayout + PersistLayout + Notify action triple,
    /// but only if `layout` differs from what is already applied.
    fn make_apply(&self, cfg: &Config, layout: &str) -> Vec<Action> {
        if self.current.as_deref() == Some(layout) {
            return vec![];
        }
        let Some(index) = cfg.layout_index(layout) else {
            eprintln!("[engine] unknown layout {layout:?} — skipping");
            return vec![];
        };
        vec![
            Action::ApplyLayout { layout: layout.to_owned(), index },
            Action::PersistLayout { layout: layout.to_owned() },
            Action::Notify { layout: layout.to_owned() },
        ]
    }

    /// Record the current layout for `class` into per-window memory.
    fn remember(&mut self, class: &str) {
        if let Some(cur) = self.current.clone() {
            self.memory.insert(class.to_owned(), cur);
        }
    }

    // ------------------------------------------------------------------
    // Forced context stack
    // ------------------------------------------------------------------

    fn push_forced(&mut self, cfg: &Config, source: &str, layout: &str) -> Vec<Action> {
        if self.forced_stack.is_empty() {
            // Save the current free layout so we can restore it later.
            self.saved = self.current.clone();
        }
        self.forced_stack.push((source.to_owned(), layout.to_owned()));
        self.make_apply(cfg, layout)
    }

    fn pop_forced(&mut self, cfg: &Config, source: &str) -> Vec<Action> {
        // Remove the frame for this specific source (it may not be the top).
        self.forced_stack.retain(|(id, _)| id != source);

        let restore = self.forced_stack.last()
            .map(|(_, l)| l.clone())
            .or_else(|| self.saved.take());

        match restore {
            Some(layout) => self.make_apply(cfg, &layout),
            None         => vec![],
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Initialise the engine from a persisted layout (read from state file).
    pub fn init(&mut self, layout: String) {
        self.current = Some(layout);
    }

    /// Process one input event and return the actions to perform.
    pub fn process(&mut self, cfg: &Config, input: EngineInput) -> Vec<Action> {
        match input {
            // ----------------------------------------------------------
            EngineInput::WindowFocus { class } => {
                // Empty class = focus moved to the desktop / no window.
                // Hyprland sends this on workspace switch or when all windows
                // are closed. Ignore it entirely — don't touch memory or
                // the forced stack, don't switch layout.
                if class.is_empty() {
                    return vec![];
                }

                // 1. Record layout for the *departing* window.
                if cfg.general.per_window_memory {
                    if let Some(old) = self.focused_class.take() {
                        if self.forced_stack.is_empty() {
                            self.remember(&old);
                        }
                    }
                }
                self.focused_class = Some(class.clone());

                // 2. Pop any forced context that was pushed for a previous
                //    layer (layers emit LayerClose, but window-focus-based
                //    forced contexts need to be cleared here).
                //    We only pop window-class-sourced frames, not layer frames.
                let was_window_forced = self.forced_stack.iter()
                    .any(|(id, _)| !id.starts_with("layer:"));
                if was_window_forced {
                    self.forced_stack.retain(|(id, _)| id.starts_with("layer:"));
                    if self.forced_stack.is_empty() {
                        // If we also cleared all layer frames, restore saved.
                        // (Rare: window closed without a LayerClose.)
                    }
                }

                match rules::match_class(cfg, &class) {
                    RuleMatch::Forced(layout) => {
                        self.push_forced(cfg, &format!("window:{class}"), &layout)
                    }
                    RuleMatch::Free => {
                        // Restore saved layout if we were forced before.
                        let restore = if !self.forced_stack.is_empty() {
                            None // still in a layer-forced context
                        } else if cfg.general.per_window_memory {
                            self.memory.get(&class).cloned()
                        } else {
                            self.saved.take()
                        };

                        match restore {
                            Some(layout) => self.make_apply(cfg, &layout),
                            None         => vec![],
                        }
                    }
                }
            }

            // ----------------------------------------------------------
            EngineInput::LayerOpen { name } => {
                match rules::match_layer(cfg, &name) {
                    RuleMatch::Forced(layout) => {
                        self.push_forced(cfg, &format!("layer:{name}"), &layout)
                    }
                    RuleMatch::Free => vec![],
                }
            }

            // ----------------------------------------------------------
            EngineInput::LayerClose { name } => {
                self.pop_forced(cfg, &format!("layer:{name}"))
            }

            // ----------------------------------------------------------
            EngineInput::Hotkey => {
                // Hotkey always resets forced context and rotates freely.
                self.forced_stack.clear();
                self.saved = None;

                let layouts = &cfg.keyboard.layouts;
                if layouts.is_empty() {
                    return vec![];
                }
                let cur = self.current.as_deref().unwrap_or("");
                let next_idx = (cfg.layout_index(cur).unwrap_or(0) + 1) % layouts.len();
                let next = layouts[next_idx].clone();

                // Update per-window memory for the focused class.
                if cfg.general.per_window_memory {
                    if let Some(class) = &self.focused_class.clone() {
                        self.memory.insert(class.clone(), next.clone());
                    }
                }

                self.make_apply(cfg, &next)
            }

            // ----------------------------------------------------------
            EngineInput::SwitchTo { layout } => {
                self.forced_stack.clear();
                self.saved = None;
                self.make_apply(cfg, &layout)
            }

            // ----------------------------------------------------------
            EngineInput::ExternalSync { layout } => {
                // Another process changed the layout. Update our state so
                // future transitions are computed from the correct baseline.
                if self.current.as_deref() != Some(&layout) {
                    eprintln!(
                        "[engine] external sync: {:?} → {:?}",
                        self.current.as_deref().unwrap_or("?"),
                        layout
                    );
                    self.current = Some(layout.clone());
                    // Also update saved/memory if we are in a free context.
                    if self.forced_stack.is_empty() {
                        if let Some(class) = &self.focused_class.clone() {
                            if cfg.general.per_window_memory {
                                self.memory.insert(class.clone(), layout.clone());
                            }
                        }
                    }
                    // Persist the synced layout.
                    return vec![Action::PersistLayout { layout }];
                }
                vec![]
            }
        }
    }

    /// Apply the action list to `self.current` and `self.saved` fields,
    /// so our internal state stays consistent.
    ///
    /// This must be called *after* the runner successfully applies the actions.
    pub fn commit(&mut self, actions: &[Action]) {
        for a in actions {
            if let Action::ApplyLayout { layout, .. } = a {
                self.current = Some(layout.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Config, ForceLayoutConfig, ForceRule, GeneralConfig, KeyboardConfig,
    };

    fn cfg_with_rules(rules: Vec<ForceRule>) -> Config {
        Config {
            keyboard: KeyboardConfig {
                device:  "kbd".into(),
                layouts: vec!["us".into(), "ru".into()],
            },
            force_layout: ForceLayoutConfig { rules },
            general: GeneralConfig {
                per_window_memory: false,
                switch_delay_ms:   0,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn base_cfg() -> Config {
        cfg_with_rules(vec![ForceRule {
            layout:         "us".into(),
            apps:           vec!["nvim".into()],
            layers:         vec!["rofi".into()],
            layer_contains: vec![],
        }])
    }

    /// Helper: apply input, commit, return actions.
    fn step(engine: &mut Engine, cfg: &Config, input: EngineInput) -> Vec<Action> {
        let actions = engine.process(cfg, input);
        engine.commit(&actions);
        actions
    }

    fn layout_applied(actions: &[Action]) -> Option<&str> {
        actions.iter().find_map(|a| {
            if let Action::ApplyLayout { layout, .. } = a { Some(layout.as_str()) } else { None }
        })
    }

    #[test]
    fn hotkey_rotates() {
        let cfg = base_cfg();
        let mut e = Engine::default();
        e.init("us".into());
        let actions = step(&mut e, &cfg, EngineInput::Hotkey);
        assert_eq!(layout_applied(&actions), Some("ru"));
        let actions = step(&mut e, &cfg, EngineInput::Hotkey);
        assert_eq!(layout_applied(&actions), Some("us"));
    }

    #[test]
    fn force_on_window_and_restore() {
        let cfg = base_cfg();
        let mut e = Engine::default();
        e.init("ru".into());

        // Focus nvim → force us
        let actions = step(&mut e, &cfg, EngineInput::WindowFocus { class: "nvim".into() });
        assert_eq!(layout_applied(&actions), Some("us"));

        // Focus firefox → free, restore ru
        let actions = step(&mut e, &cfg, EngineInput::WindowFocus { class: "firefox".into() });
        assert_eq!(layout_applied(&actions), Some("ru"));
    }

    #[test]
    fn layer_force_nested_in_window_force() {
        let cfg = base_cfg();
        let mut e = Engine::default();
        e.init("ru".into());

        // Focus nvim → us (window forced)
        step(&mut e, &cfg, EngineInput::WindowFocus { class: "nvim".into() });

        // Open rofi → still us (layer forced, same layout)
        let actions = step(&mut e, &cfg, EngineInput::LayerOpen { name: "rofi".into() });
        assert!(layout_applied(&actions).is_none()); // no change needed

        // Close rofi → back to window force (nvim → us)
        let actions = step(&mut e, &cfg, EngineInput::LayerClose { name: "rofi".into() });
        assert!(layout_applied(&actions).is_none()); // still us from window force

        // Focus firefox → restore ru
        let actions = step(&mut e, &cfg, EngineInput::WindowFocus { class: "firefox".into() });
        assert_eq!(layout_applied(&actions), Some("ru"));
    }

    #[test]
    fn external_sync_updates_baseline() {
        let cfg = base_cfg();
        let mut e = Engine::default();
        e.init("us".into());

        // External tool switched to ru.
        let actions = step(&mut e, &cfg, EngineInput::ExternalSync { layout: "ru".into() });
        // Only PersistLayout, no ApplyLayout (we don't re-apply what's already active).
        assert!(layout_applied(&actions).is_none());
        assert!(actions.iter().any(|a| matches!(a, Action::PersistLayout { .. })));

        // Now hotkey should rotate from ru → us (not us → ru).
        let actions = step(&mut e, &cfg, EngineInput::Hotkey);
        assert_eq!(layout_applied(&actions), Some("us"));
    }

    #[test]
    fn no_redundant_apply() {
        let cfg = base_cfg();
        let mut e = Engine::default();
        e.init("us".into());

        // Force us again — should produce no actions.
        let actions = step(&mut e, &cfg, EngineInput::WindowFocus { class: "nvim".into() });
        assert!(layout_applied(&actions).is_none());
    }
}
