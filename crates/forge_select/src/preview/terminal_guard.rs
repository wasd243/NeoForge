use std::io;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode};

/// RAII guard that configures the terminal for the selector UI and restores
/// the previous terminal state on drop.
///
/// On entry it enables raw mode, enables mouse capture, and hides the cursor.
/// On drop it shows the cursor, disables mouse capture, and disables raw mode
/// only if raw mode was not already enabled before entering.
pub(super) struct TerminalGuard {
    raw_mode_was_enabled: bool,
}

impl TerminalGuard {
    /// Enters the selector terminal state and returns a guard that restores
    /// the previous state when dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if querying or enabling raw mode fails, or if the
    /// terminal commands for mouse capture and cursor visibility cannot be
    /// written to stderr.
    pub(super) fn enter() -> anyhow::Result<Self> {
        let raw_mode_was_enabled = terminal::is_raw_mode_enabled()?;
        enable_raw_mode()?;
        execute!(io::stderr(), EnableMouseCapture, Hide)?;
        Ok(Self { raw_mode_was_enabled })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stderr(), Show, DisableMouseCapture);
        if !self.raw_mode_was_enabled {
            let _ = disable_raw_mode();
        }
    }
}
