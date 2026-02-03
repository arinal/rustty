//! Terminal emulator module
//!
//! This module contains all terminal emulation functionality including:
//! - ANSI escape sequence parsing and command types
//! - Terminal grid with scrollback and alternate screen
//! - Color representation and palette
//! - Cursor positioning
//! - Terminal state management
//! - VTE parser integration

// Submodules
pub mod command;
pub mod color;
pub mod cursor;
pub mod grid;
pub mod state;

// Re-export commonly used types
pub use command::{AnsiParseError, CsiCommand, DecPrivateMode, EraseMode, SgrParameter};
pub use color::Color;
pub use cursor::Cursor;
pub use grid::{Cell, TerminalGrid};
pub use state::TerminalState;

use vte::{Params, Parser, Perform};

/// Terminal emulator
///
/// Combines VTE parser state with terminal emulator state.
/// Provides a clean API for processing input bytes and accessing terminal state.
pub struct Terminal {
    /// Terminal state (grid, cursor, colors, attributes)
    state: TerminalState,
    /// VTE parser state machine
    parser: Parser,
}

impl Terminal {
    /// Create a new terminal with the given dimensions
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            state: TerminalState::new(cols, rows),
            parser: Parser::new(),
        }
    }

    /// Process input bytes through the VTE parser
    ///
    /// This parses ANSI escape sequences and updates the terminal state accordingly.
    pub fn process_bytes(&mut self, bytes: &[u8]) {
        // Temporarily take ownership of the parser to avoid borrow checker issues
        let mut parser = std::mem::replace(&mut self.parser, Parser::new());
        for &byte in bytes {
            parser.advance(self, byte);
        }
        self.parser = parser;
    }

    /// Get immutable reference to terminal state
    pub fn state(&self) -> &TerminalState {
        &self.state
    }

    /// Get mutable reference to terminal state
    pub fn state_mut(&mut self) -> &mut TerminalState {
        &mut self.state
    }

    /// Resize the terminal grid
    ///
    /// Preserves existing content and clamps cursor to valid position.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.state.grid.resize(cols, rows);

        // Clamp cursor to valid position
        self.state.cursor.row = self.state.cursor.row.min(rows.saturating_sub(1));
        self.state.cursor.col = self.state.cursor.col.min(cols.saturating_sub(1));
    }

    /// Get a parameter from a CSI sequence, with a default value if not present
    #[inline]
    fn param_or(&self, params: &Params, index: usize, default: u16) -> u16 {
        params
            .iter()
            .nth(index)
            .and_then(|p| p.first())
            .copied()
            .unwrap_or(default)
    }

    /// Get next parameter value from an iterator with a default
    #[inline]
    fn next_param<'a>(iter: &mut impl Iterator<Item = &'a [u16]>, default: u16) -> u16 {
        iter.next()
            .and_then(|p| p.first())
            .copied()
            .unwrap_or(default)
    }

    /// Extract RGB values from parameter iterator
    #[inline]
    fn extract_rgb<'a>(iter: &mut impl Iterator<Item = &'a [u16]>) -> (u8, u8, u8) {
        let r = Self::next_param(iter, 0) as u8;
        let g = Self::next_param(iter, 0) as u8;
        let b = Self::next_param(iter, 0) as u8;
        (r, g, b)
    }

    /// Handle extended color sequences (38/48 SGR codes)
    fn handle_extended_color<'a>(
        iter: &mut impl Iterator<Item = &'a [u16]>,
        is_foreground: bool,
        fg: &mut Color,
        bg: &mut Color,
    ) {
        if let Some(next_param) = iter.next() {
            match next_param.first().copied().unwrap_or(0) {
                2 => {
                    // RGB color
                    let (r, g, b) = Self::extract_rgb(iter);
                    let color = Color::new(r, g, b);
                    if is_foreground {
                        *fg = color;
                    } else {
                        *bg = color;
                    }
                }
                5 => {
                    // 256-color palette (full 0-255 range)
                    let idx = Self::next_param(iter, 0) as u8;
                    let color = Color::from_ansi_index(idx);
                    if is_foreground {
                        *fg = color;
                    } else {
                        *bg = color;
                    }
                }
                _ => {}
            }
        }
    }

    /// Handle DEC private mode set (ESC[?{mode}h)
    fn handle_dec_mode_set(&mut self, params: &Params) {
        let mode_num = self.param_or(params, 0, 0);
        let mode = DecPrivateMode::from_mode(mode_num);

        match mode {
            DecPrivateMode::AlternateScreenBuffer => {
                // Enable alternate screen buffer + save cursor
                self.state.grid.use_alternate_screen();
                // Clear the alternate screen
                self.state.grid.clear_viewport();
                self.state.cursor.row = 0;
                self.state.cursor.col = 0;
            }
            DecPrivateMode::Unknown(mode) => {
                eprintln!("[ANSI] Unknown DEC private mode (set): {}", mode);
            }
            _ => {
                eprintln!(
                    "[ANSI] Not yet implemented DEC private mode (set): {:?}",
                    mode
                );
            }
        }
    }

    /// Handle DEC private mode reset (ESC[?{mode}l)
    fn handle_dec_mode_reset(&mut self, params: &Params) {
        let mode_num = self.param_or(params, 0, 0);
        let mode = DecPrivateMode::from_mode(mode_num);

        match mode {
            DecPrivateMode::AlternateScreenBuffer => {
                // Restore main screen buffer
                self.state.grid.use_main_screen();
            }
            DecPrivateMode::Unknown(mode) => {
                eprintln!("[ANSI] Unknown DEC private mode (reset): {}", mode);
            }
            _ => {
                eprintln!(
                    "[ANSI] Not yet implemented DEC private mode (reset): {:?}",
                    mode
                );
            }
        }
    }

    /// Handle SGR (Select Graphic Rendition) parameters
    fn handle_sgr(&mut self, params: &Params) {
        // If no parameters, default to reset (0)
        if params.is_empty() {
            self.state.fg = Color::white();
            self.state.bg = Color::black();
            self.state.bold = false;
            self.state.italic = false;
            self.state.underline = false;
            return;
        }

        let mut iter = params.iter();
        while let Some(param) = iter.next() {
            let code = param.first().copied().unwrap_or(0);
            let sgr = SgrParameter::from_code(code);

            match sgr {
                SgrParameter::Reset => {
                    self.state.fg = Color::white();
                    self.state.bg = Color::black();
                    self.state.bold = false;
                    self.state.italic = false;
                    self.state.underline = false;
                }
                SgrParameter::Bold => {
                    self.state.bold = true;
                }
                SgrParameter::Italic => {
                    self.state.italic = true;
                }
                SgrParameter::Underline => {
                    self.state.underline = true;
                }
                SgrParameter::NormalIntensity => {
                    self.state.bold = false;
                }
                SgrParameter::NotItalic => {
                    self.state.italic = false;
                }
                SgrParameter::NotUnderlined => {
                    self.state.underline = false;
                }
                SgrParameter::ForegroundColor(idx) => {
                    self.state.fg = Color::from_ansi_index(idx);
                }
                SgrParameter::BackgroundColor(idx) => {
                    self.state.bg = Color::from_ansi_index(idx);
                }
                SgrParameter::BrightForegroundColor(idx) => {
                    self.state.fg = Color::from_ansi_index(idx + 8);
                }
                SgrParameter::BrightBackgroundColor(idx) => {
                    self.state.bg = Color::from_ansi_index(idx + 8);
                }
                SgrParameter::DefaultForeground => {
                    self.state.fg = Color::white();
                }
                SgrParameter::DefaultBackground => {
                    self.state.bg = Color::black();
                }
                SgrParameter::ExtendedForeground => {
                    Self::handle_extended_color(&mut iter, true, &mut self.state.fg, &mut self.state.bg);
                }
                SgrParameter::ExtendedBackground => {
                    Self::handle_extended_color(&mut iter, false, &mut self.state.fg, &mut self.state.bg);
                }
                SgrParameter::Unknown(code) => {
                    eprintln!("[ANSI] Unknown SGR parameter: {}", code);
                }
                _ => {
                    eprintln!("[ANSI] Not yet implemented SGR: {:?}", sgr);
                }
            }
        }
    }
}

impl Perform for Terminal {
    fn print(&mut self, c: char) {
        // Create cell with current attributes
        let cell = Cell {
            ch: c,
            fg: self.state.fg,
            bg: self.state.bg,
            bold: self.state.bold,
            italic: self.state.italic,
            underline: self.state.underline,
        };

        // Check if we need to wrap to next line
        if self.state.cursor.col >= self.state.grid.width {
            self.state.cursor.col = 0;
            self.state.cursor.row += 1;

            // If at bottom, scroll the viewport down
            if self.state.cursor.row >= self.state.grid.viewport_height {
                self.state.cursor.row = self.state.grid.viewport_height - 1;
                // TODO: Actual scrolling logic
            }
        }

        // Put the cell at cursor position
        self.state.grid.put_cell(cell, self.state.cursor.row, self.state.cursor.col);

        // Move cursor forward
        self.state.cursor.col += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                // Line Feed (LF) - move down one line
                self.state.cursor.row += 1;
                if self.state.cursor.row >= self.state.grid.viewport_height {
                    self.state.cursor.row = self.state.grid.viewport_height - 1;
                    // TODO: Actual scrolling logic
                }
            }
            b'\r' => {
                // Carriage Return (CR) - move to start of line
                self.state.cursor.col = 0;
            }
            b'\x08' => {
                // Backspace
                if self.state.cursor.col > 0 {
                    self.state.cursor.col -= 1;
                }
            }
            b'\t' => {
                // Tab - move to next tab stop (every 8 columns)
                let next_tab = ((self.state.cursor.col / 8) + 1) * 8;
                self.state.cursor.col = next_tab.min(self.state.grid.width - 1);
            }
            _ => {
                // Other control characters - ignore for now
            }
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        // Check if this is a DEC private mode sequence (starts with '?')
        let is_dec_private = intermediates.first() == Some(&b'?');

        if is_dec_private {
            // Handle DEC private modes
            match action {
                'h' => self.handle_dec_mode_set(params),
                'l' => self.handle_dec_mode_reset(params),
                _ => {
                    eprintln!("[ANSI] Unknown DEC private mode action: {}", action);
                }
            }
            return;
        }

        // Parse CSI command with parameters
        let command = match CsiCommand::parse(action, params, is_dec_private) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("[ANSI] Failed to parse CSI command: {:?}", e);
                return;
            }
        };

        // Handle SGR specially since it has variable parameters
        if let CsiCommand::SelectGraphicRendition = command {
            self.handle_sgr(params);
            return;
        }

        // Execute command by pattern matching on enum variants
        match command {
            // Cursor positioning
            CsiCommand::CursorPosition { row, col } => {
                self.state.cursor.row =
                    (row.saturating_sub(1) as usize).min(self.state.grid.viewport_height - 1);
                self.state.cursor.col = (col.saturating_sub(1) as usize).min(self.state.grid.width - 1);
            }

            // Cursor movement
            CsiCommand::CursorUp { n } => {
                self.state.cursor.row = self.state.cursor.row.saturating_sub(n as usize);
            }

            CsiCommand::CursorDown { n } => {
                self.state.cursor.row = (self.state.cursor.row + n as usize).min(self.state.grid.viewport_height - 1);
            }

            CsiCommand::CursorForward { n } => {
                self.state.cursor.col = (self.state.cursor.col + n as usize).min(self.state.grid.width - 1);
            }

            CsiCommand::CursorBack { n } => {
                self.state.cursor.col = self.state.cursor.col.saturating_sub(n as usize);
            }

            CsiCommand::CursorHorizontalAbsolute { col } => {
                self.state.cursor.col = (col.saturating_sub(1) as usize).min(self.state.grid.width - 1);
            }

            // Erase operations
            CsiCommand::EraseInDisplay { mode } => match mode {
                EraseMode::ToEnd => {
                    self.state.grid.clear_line(self.state.cursor.row);
                }
                EraseMode::All => {
                    self.state.grid.clear_viewport();
                    self.state.cursor.row = 0;
                    self.state.cursor.col = 0;
                }
                EraseMode::ToBeginning => {
                    // Clear from beginning to cursor
                    for row in 0..self.state.cursor.row {
                        self.state.grid.clear_line(row);
                    }
                    // Clear current line up to cursor
                    for col in 0..=self.state.cursor.col {
                        self.state.grid.put_cell(Cell::default(), self.state.cursor.row, col);
                    }
                }
                EraseMode::AllWithScrollback => {
                    // Clear scrollback history
                    // This would require additional grid methods
                    // For now, just clear viewport
                    self.state.grid.clear_viewport();
                }
            },

            CsiCommand::EraseInLine { mode } => match mode {
                EraseMode::ToEnd => {
                    for col in self.state.cursor.col..self.state.grid.width {
                        self.state.grid.put_cell(Cell::default(), self.state.cursor.row, col);
                    }
                }
                EraseMode::All => {
                    self.state.grid.clear_line(self.state.cursor.row);
                }
                EraseMode::ToBeginning => {
                    for col in 0..=self.state.cursor.col {
                        self.state.grid.put_cell(Cell::default(), self.state.cursor.row, col);
                    }
                }
                EraseMode::AllWithScrollback => {
                    // Not applicable to EraseInLine
                }
            },

            // Scrolling region
            CsiCommand::SetScrollingRegion { top, bottom } => {
                // Store scrolling region in state
                // This requires adding scrolling region fields to TerminalState
                // For now, this is a no-op
                let _ = (top, bottom);
            }

            // Line manipulation
            CsiCommand::InsertLines { n } => {
                // Insert n blank lines at cursor position
                // This requires grid method for line insertion
                let _ = n;
            }

            CsiCommand::DeleteLines { n } => {
                // Delete n lines at cursor position
                // This requires grid method for line deletion
                let _ = n;
            }

            // Already handled above
            CsiCommand::SelectGraphicRendition => {}

            // Device queries and window manipulation
            CsiCommand::DeviceStatusReport { .. } => {
                // Would need to send response to PTY
                // Not implementable at this level
            }

            CsiCommand::DeviceAttributes { .. } => {
                // Would need to send response to PTY
                // Not implementable at this level
            }

            CsiCommand::WindowManipulation { .. } => {
                // Window operations (resize, minimize, etc.)
                // Not implementable at core level
            }

            CsiCommand::SetCursorStyle { .. } => {
                // Set cursor style (block, underline, bar)
                // Requires cursor style in state
            }

            CsiCommand::Unknown(_) => {
                // No-op for unknown commands
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_new() {
        let terminal = Terminal::new(80, 24);

        assert_eq!(terminal.state().grid.width, 80);
        assert_eq!(terminal.state().grid.viewport_height, 24);
        assert_eq!(terminal.state().cursor.row, 0);
        assert_eq!(terminal.state().cursor.col, 0);
    }

    #[test]
    fn test_terminal_process_bytes() {
        let mut terminal = Terminal::new(80, 24);

        // Process "Hello"
        terminal.process_bytes(b"Hello");

        // Check that characters were written
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'H');
        assert_eq!(viewport[0][1].ch, 'e');
        assert_eq!(viewport[0][2].ch, 'l');
        assert_eq!(viewport[0][3].ch, 'l');
        assert_eq!(viewport[0][4].ch, 'o');

        assert_eq!(terminal.state().cursor.col, 5);
    }

    #[test]
    fn test_terminal_ansi_escape_sequence() {
        let mut terminal = Terminal::new(80, 24);

        // Process cursor movement: ESC[10;20H
        terminal.process_bytes(b"\x1b[10;20H");

        // Cursor should be at (9, 19) in 0-indexed coordinates
        assert_eq!(terminal.state().cursor.row, 9);
        assert_eq!(terminal.state().cursor.col, 19);
    }

    #[test]
    fn test_terminal_resize() {
        let mut terminal = Terminal::new(80, 24);

        // Set cursor near edge
        terminal.state_mut().cursor.row = 20;
        terminal.state_mut().cursor.col = 70;

        // Resize to smaller dimensions
        terminal.resize(50, 15);

        // Check grid resized
        assert_eq!(terminal.state().grid.width, 50);
        assert_eq!(terminal.state().grid.viewport_height, 15);

        // Check cursor clamped
        assert_eq!(terminal.state().cursor.row, 14);
        assert_eq!(terminal.state().cursor.col, 49);
    }

    #[test]
    fn test_terminal_colors() {
        let mut terminal = Terminal::new(80, 24);

        // Set foreground color to red (ANSI 31)
        terminal.process_bytes(b"\x1b[31m");

        // Foreground should be red (xterm color palette: RGB(205, 49, 49))
        let fg = &terminal.state().fg;
        assert_eq!(fg.r, 205);
        assert_eq!(fg.g, 49);
        assert_eq!(fg.b, 49);
    }
}
