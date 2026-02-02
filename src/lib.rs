//! Rustty - Terminal Emulation Library
//!
//! This library provides terminal emulation functionality including:
//! - PTY (pseudo-terminal) management via Shell
//! - ANSI escape sequence parsing and handling
//! - Terminal grid with scrollback and alternate screen buffer
//! - Color support (256-color palette + RGB true color)
//!
//! This library has zero UI dependencies - it only handles terminal logic.
//! For a complete terminal emulator application, see the `rustty` binary.

// Terminal emulation module (all terminal-related functionality)
pub mod terminal;

// Re-export commonly used terminal types
pub use terminal::{
    AnsiParseError, Cell, Color, CsiCommand, Cursor, DecPrivateMode, EraseMode, SgrParameter,
    Shell, Terminal, TerminalGrid, TerminalState,
};
