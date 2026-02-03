//! Shell management with PTY
//!
//! This module provides the Shell abstraction which manages a shell process
//! running in a pseudo-terminal (PTY), including process lifecycle,
//! communication channels, and background I/O.

use anyhow::Result;
use nix::libc;
use nix::pty::{Winsize, openpty};
use nix::unistd::{ForkResult, fork};
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

/// Shell process with PTY and background I/O
///
/// Manages a shell process running in a pseudo-terminal, including
/// automatic background reading via a dedicated thread. Output from
/// the shell is available through the receiver channel.
pub struct Shell {
    master: Arc<OwnedFd>,
    /// Receiver for shell output from the background reader thread
    pub receiver: Receiver<Vec<u8>>,
}

/// Iterator for reading from PTY in a background thread.
///
/// The iterator yields chunks of data as `Vec<u8>` and automatically
/// handles EOF and errors by returning `None`. The file descriptor
/// is kept alive through Arc reference counting, ensuring it stays
/// open as long as any PtyReader exists.
struct PtyReader {
    master: Arc<OwnedFd>,
}

impl Iterator for PtyReader {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; 4096];

        match nix::unistd::read(self.master.as_raw_fd(), &mut buf) {
            Ok(0) => None, // EOF - shell exited
            Ok(n) => Some(buf[..n].to_vec()),
            Err(_) => None, // Error reading
        }
    }
}

impl Shell {
    /// Spawn a new shell process with PTY and background reader
    ///
    /// Creates a new shell process running in a pseudo-terminal with the
    /// specified dimensions. The shell is determined by the SHELL environment
    /// variable, defaulting to /bin/sh. A background thread is automatically
    /// spawned to read shell output, which can be accessed via the receiver.
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let pty_result = openpty(Some(&winsize), None)?;

        // Fork the process
        match unsafe { fork()? } {
            ForkResult::Parent { .. } => {
                // Parent process - we are the terminal emulator
                // Close the slave side in the parent
                drop(pty_result.slave);

                let master = Arc::new(pty_result.master);

                // Set up channel for shell output
                let (tx, rx) = channel();

                // Create reader iterator (Arc clones the FD reference)
                let reader = PtyReader {
                    master: Arc::clone(&master),
                };

                // Spawn reader thread with iterator pattern
                thread::spawn(move || {
                    for data in reader {
                        if tx.send(data).is_err() {
                            // Main thread has dropped the receiver, exit
                            break;
                        }
                    }
                    // Reader iterator ended (EOF or error)
                    // Arc cleanup happens automatically when reader is dropped
                });

                Ok(Shell {
                    master,
                    receiver: rx,
                })
            }
            ForkResult::Child => {
                // Child process - we will become the shell
                // Close master in child
                drop(pty_result.master);

                // Create a new session
                nix::unistd::setsid()?;

                let slave_fd = pty_result.slave.as_raw_fd();

                // Make the slave the controlling terminal
                unsafe {
                    libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);
                }

                // Duplicate slave to stdin, stdout, stderr
                nix::unistd::dup2(slave_fd, 0)?; // stdin
                nix::unistd::dup2(slave_fd, 1)?; // stdout
                nix::unistd::dup2(slave_fd, 2)?; // stderr

                // Close the original slave fd
                drop(pty_result.slave);

                // Execute the shell
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                let shell_cstr = std::ffi::CString::new(shell.as_str())?;
                nix::unistd::execvp(&shell_cstr, std::slice::from_ref(&shell_cstr))?;

                // If exec fails, exit
                std::process::exit(1);
            }
        }
    }

    /// Write data to the shell's input
    pub fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        nix::unistd::write(self.master.as_ref(), buf).map_err(|e| e.into())
    }

    /// Resize the pseudo-terminal window
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe {
            if libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) == -1 {
                // Capture the actual error from errno
                let err = std::io::Error::last_os_error();
                return Err(anyhow::Error::new(err).context("Failed to set PTY window size"));
            }
        }

        Ok(())
    }
}
