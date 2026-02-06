//! Rustty - Terminal Emulation Library
//!
//! This library provides terminal emulation functionality including:
//! - PTY (pseudo-terminal) management via Shell
//! - ANSI escape sequence parsing and handling
//! - Terminal grid with scrollback and alternate screen buffer
//! - Color support (256-color palette + RGB true color)
//! - Application facade with rendering backends (CPU via Raqote, GPU via wgpu)
//! - Terminal session orchestration without UI dependencies
//!
//! ## Architecture
//!
//! Rustty is organized into distinct layers:
//! - **Terminal primitives** - Core emulation (`Terminal`, `Shell`, `TerminalState`)
//! - **Session layer** - Terminal-only facade without UI (`TerminalSession`)
//! - **Application layer** - Full UI facade with renderer abstraction (`App<R>`)
//! - **Renderer backends** - CPU (Raqote) and GPU (wgpu) implementations
//!
//! ## Quick Start
//!
//! ### For Terminal Emulator Applications (with UI)
//!
//! Use `App<R>` with a renderer backend:
//!
//! ```no_run
//! use rustty::{App, renderer::CpuRenderer};
//!
//! let mut app: App<CpuRenderer> = App::new();
//! // Initialize renderer and window
//! // Handle events, keyboard input, mouse, rendering
//! ```
//!
//! ### For Terminal-Only Applications (no UI)
//!
//! Use `TerminalSession` for applications that need terminal emulation
//! without a full UI:
//!
//! ```no_run
//! use rustty::TerminalSession;
//!
//! let mut session = TerminalSession::new(80, 24)?;
//!
//! // Check for shell output and update terminal
//! while session.process_output() {
//!     // Get viewport for custom rendering
//!     let viewport = session.state().grid.get_viewport();
//!     // ... render to screen with your own code
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```

// Shell process and PTY management
pub mod shell;

// Terminal emulation module (all terminal-related functionality)
pub mod terminal;

// Rendering backends (CPU and GPU)
pub mod renderer;

// Application state and input handling
pub mod app;

// Terminal session management
pub mod session;

// Re-export commonly used types
pub use app::{App, AppBase};
pub use session::TerminalSession;
pub use shell::Shell;
pub use terminal::{
    AnsiParseError, Cell, Color, CsiCommand, Cursor, CursorStyle, DecPrivateMode, EraseMode,
    SgrParameter, Terminal, TerminalGrid, TerminalState,
};
