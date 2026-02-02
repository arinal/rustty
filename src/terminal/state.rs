//! Terminal state management
//!
//! This module contains the TerminalState struct which holds all mutable
//! state for the terminal emulator as a pure data structure.

use super::color::Color;
use super::cursor::Cursor;
use super::grid::TerminalGrid;

/// Terminal state
///
/// Contains all mutable state for the terminal including the grid,
/// cursor position, and text attributes. This is a pure data structure
/// without any behavior - the parsing logic lives in Terminal.
pub struct TerminalState {
    /// The terminal grid (cells, scrollback, alternate screen)
    pub grid: TerminalGrid,

    /// Cursor (position, visibility, style)
    pub cursor: Cursor,

    /// Foreground color
    pub fg: Color,

    /// Background color
    pub bg: Color,

    /// Bold attribute
    pub bold: bool,

    /// Italic attribute
    pub italic: bool,

    /// Underline attribute
    pub underline: bool,
}

impl TerminalState {
    /// Create a new terminal state with the given dimensions
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            grid: TerminalGrid::new(cols, rows),
            cursor: Cursor::at_origin(),
            fg: Color::white(),
            bg: Color::black(),
            bold: false,
            italic: false,
            underline: false,
        }
    }
}
