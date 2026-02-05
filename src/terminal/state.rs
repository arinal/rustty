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

    /// Reverse video attribute (swap fg/bg colors)
    pub reverse: bool,

    /// Auto wrap mode - whether text wraps to next line at right margin
    pub auto_wrap: bool,

    /// Bracketed paste mode - wraps pasted text with markers
    pub bracketed_paste: bool,

    /// Application cursor keys mode - changes arrow key sequences
    pub application_cursor_keys: bool,

    /// Show cursor mode - controls cursor visibility
    pub show_cursor: bool,

    /// Cursor blink mode - controls cursor blinking
    pub cursor_blink: bool,

    /// Mouse SGR tracking mode - enables SGR mouse protocol
    pub mouse_sgr: bool,

    /// Focus events mode - sends focus in/out sequences
    pub focus_events: bool,

    /// Mouse tracking mode - enables button event reporting (mode 1000)
    pub mouse_tracking: bool,

    /// Mouse cell motion mode - enables button + drag reporting (mode 1002)
    pub mouse_cell_motion: bool,
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
            reverse: false,
            auto_wrap: true, // VT100 default
            bracketed_paste: false,
            application_cursor_keys: false,
            show_cursor: true,   // Cursor visible by default
            cursor_blink: false, // No blinking by default
            mouse_sgr: false,
            focus_events: false,
            mouse_tracking: false,
            mouse_cell_motion: false,
        }
    }
}
