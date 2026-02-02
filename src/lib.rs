//! Library exports for Rustty terminal emulator
//! This allows examples and tests to access the internal modules

// Terminal emulation module (all terminal-related functionality)
pub mod terminal;

// Application module
pub mod app; // winit + softbuffer + raqote adapter

// Re-export commonly used terminal types
pub use terminal::{
    AnsiParseError, Cell, Color, CsiCommand, Cursor, DecPrivateMode, EraseMode, SgrParameter,
    Shell, Terminal, TerminalGrid, TerminalState,
};

// Re-export app type
pub use app::App;
