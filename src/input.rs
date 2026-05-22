//! Raw input backend abstraction.

pub mod evdev;

/// A source of hotkey events. Runs in its own thread.
pub trait InputBackend: Send + 'static {
    /// Block until a hotkey fires, then return. `None` = permanent failure.
    fn next_hotkey(&mut self) -> Option<()>;
}
