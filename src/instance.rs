//! Single-instance guard.
//!
//! Uses a PID file in `$XDG_RUNTIME_DIR`. On acquire:
//!
//! 1. If no PID file → write ours → proceed.
//! 2. If PID file exists but process is dead → overwrite → proceed.
//! 3. If process is alive → error.
//!
//! Dropping `InstanceGuard` removes the PID file.

use std::{fs, io, path::PathBuf, process};

const PID_FILENAME: &str = "hyprxkb.pid";

fn pid_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join(PID_FILENAME)
}

fn process_alive(pid: u32) -> bool {
    // `kill(pid, 0)` checks existence without sending a signal.
    // SAFETY: standard libc call.
    let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if ret == 0 {
        return true;
    }
    let err = io::Error::last_os_error();
    // EPERM = process exists but owned by another user → treat as alive.
    err.raw_os_error() == Some(libc::EPERM)
}

/// RAII guard that holds the PID lock file.
pub struct InstanceGuard {
    path: PathBuf,
}

impl InstanceGuard {
    /// Attempt to acquire the single-instance lock.
    ///
    /// Returns `Err` if another instance is already running.
    pub fn acquire() -> io::Result<Self> {
        let path = pid_path();

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                if process_alive(pid) {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "hyprxkb is already running (PID {pid}). \
                             Use `hyprxkb reload` to reload config.",
                        ),
                    ));
                }
                eprintln!("[instance] stale lock for dead PID {pid} — removing");
            }
        }

        fs::write(&path, process::id().to_string())?;
        Ok(Self { path })
    }

    /// Path to the PID file (useful for CLI commands that need to read it).
    pub fn pid_file_path() -> PathBuf {
        pid_path()
    }

    /// Read the PID of the running instance (if any).
    pub fn running_pid() -> Option<u32> {
        fs::read_to_string(pid_path())
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .filter(|&pid| process_alive(pid))
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
