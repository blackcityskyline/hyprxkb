//! Compositor abstraction: trait + unified event type.
//!
//! Adding Niri support = implement `Compositor` for `NiriCompositor` in a new
//! `niri.rs` submodule. No changes to the engine or main loop required.

pub mod hyprland;

// ---------------------------------------------------------------------------
// Events produced by any compositor
// ---------------------------------------------------------------------------

/// Compositor events that the layout engine cares about.
#[derive(Debug, Clone)]
pub enum CompositorEvent {
    /// A window received focus.
    WindowFocus {
        /// Window class (WM_CLASS), lowercased.
        class: String,
    },
    /// A layer surface was opened (e.g. rofi, wofi, swaylock).
    LayerOpen {
        /// Layer surface name, lowercased.
        name: String,
    },
    /// A layer surface was closed.
    LayerClose {
        /// Layer surface name, lowercased.
        name: String,
    },
}

// ---------------------------------------------------------------------------
// Compositor trait
// ---------------------------------------------------------------------------

/// A live connection to a Wayland compositor that can stream events.
///
/// Implement this trait in a new submodule to add compositor support (e.g. Niri).
#[allow(dead_code)]
pub trait Compositor: Send + 'static {
    /// Block until the next event is available, then return it.
    /// Returns `None` when the connection is permanently closed.
    fn next_event(&mut self) -> Option<CompositorEvent>;

    /// Apply a layout switch. `index` is the 0-based position in
    /// `keyboard.layouts`. Returns `Ok(())` on success.
    fn set_layout(&self, device: &str, index: usize) -> Result<(), String>;

    /// Query the currently active layout name from the compositor.
    /// Used for external-change sync. Returns `None` on failure.
    fn active_layout(&self, device: &str) -> Option<String>;
}
