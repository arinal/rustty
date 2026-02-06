//! Terminal session management
//!
//! This module provides `TerminalSession`, which combines terminal emulation
//! with shell process management for applications that need terminal functionality
//! without a full UI.

use crate::{Shell, Terminal, TerminalState};
use anyhow::Result;

/// Terminal session that orchestrates Terminal and Shell
///
/// Combines terminal emulation (ANSI parsing, grid, state) with shell process
/// management (PTY, I/O). Provides a unified interface for applications that
/// don't need to manage these components separately.
///
/// This is useful for terminal-only applications without UI. For full terminal
/// emulator applications with UI, use `App<R>` instead.
pub struct TerminalSession {
    terminal: Terminal,
    shell: Option<Shell>,
}

impl TerminalSession {
    /// Create a new terminal session with shell
    ///
    /// Spawns a shell process in a PTY with the given dimensions.
    /// The shell is determined by the SHELL environment variable,
    /// defaulting to /bin/sh.
    pub fn new(cols: usize, rows: usize) -> Result<Self> {
        let terminal = Terminal::new(cols, rows);
        let shell = Shell::new(cols as u16, rows as u16).ok();

        if shell.is_none() {
            eprintln!("Failed to create shell");
        }

        Ok(Self { terminal, shell })
    }

    /// Process shell output and update terminal state
    ///
    /// Checks for available shell output (non-blocking) and processes it
    /// through the terminal emulator. Returns false if the shell process
    /// has exited, true otherwise.
    ///
    /// Should be called regularly (e.g., in the event loop) to keep the
    /// terminal display synchronized with shell output.
    pub fn process_output(&mut self) -> bool {
        if let Some(ref mut shell) = self.shell {
            let mut has_data = false;

            // Drain all available messages from the channel
            loop {
                match shell.receiver.try_recv() {
                    Ok(data) => {
                        has_data = true;
                        // Process bytes through the terminal (VTE parser + state updates)
                        self.terminal.process_bytes(&data);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // No more data available right now
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Channel closed - child process has exited
                        eprintln!("Child process exited");
                        return false;
                    }
                }
            }

            if has_data {
                self.terminal.state_mut().grid.viewport_to_end();
            }

            // Send any pending responses back to the shell
            let responses = self.terminal.drain_responses();
            for response in responses {
                if let Err(e) = shell.write(&response) {
                    eprintln!("Failed to send response to shell: {}", e);
                }
            }
        }
        true
    }

    /// Write input bytes to the shell
    ///
    /// Sends keyboard input or other data to the shell process.
    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(shell) = &mut self.shell {
            shell.write(bytes)?;
        }
        Ok(())
    }

    /// Resize the terminal and shell
    ///
    /// Updates both the terminal grid size and the PTY window size.
    /// The terminal grid preserves existing content and clamps the cursor.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        // Resize terminal (preserves existing content and clamps cursor)
        self.terminal.resize(cols, rows);

        // Update shell PTY size
        if let Some(shell) = &mut self.shell
            && let Err(e) = shell.resize(cols as u16, rows as u16)
        {
            eprintln!("Failed to resize shell: {}", e);
        }
    }

    /// Get read-only access to terminal state
    pub fn state(&self) -> &TerminalState {
        self.terminal.state()
    }

    /// Get mutable access to terminal state
    pub fn state_mut(&mut self) -> &mut TerminalState {
        self.terminal.state_mut()
    }

    /// Check if shell is running
    pub fn has_shell(&self) -> bool {
        self.shell.is_some()
    }
}
