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
pub mod color;
pub mod command;
pub mod cursor;
pub mod grid;
pub mod state;

// Re-export commonly used types
pub use color::Color;
pub use command::{AnsiParseError, CsiCommand, DecPrivateMode, EraseMode, SgrParameter};
pub use cursor::{Cursor, CursorStyle};
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
    /// Pending responses to be sent back to the shell
    pending_responses: Vec<Vec<u8>>,
}

impl Terminal {
    /// Create a new terminal with the given dimensions
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            state: TerminalState::new(cols, rows),
            parser: Parser::new(),
            pending_responses: Vec::new(),
        }
    }

    /// Drain pending responses that need to be sent to the shell
    ///
    /// Returns a vector of byte sequences to be written to the shell.
    /// The internal buffer is cleared after this call.
    pub fn drain_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_responses)
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

    /// Generate DECRQM (DEC Request Mode) response
    ///
    /// Format: ESC[?{mode};{value}$y
    /// where value is:
    ///   0 = not recognized
    ///   1 = set
    ///   2 = reset
    ///   3 = permanently set
    ///   4 = permanently reset
    fn generate_decrqm_response(&mut self, mode_num: u16) {
        let mode = DecPrivateMode::from_mode(mode_num);

        let value = match mode {
            DecPrivateMode::AlternateScreenBuffer => {
                if self.state.grid.use_alternate_screen { 1 } else { 2 }
            }
            DecPrivateMode::AutoWrapMode => {
                if self.state.auto_wrap { 1 } else { 2 }
            }
            DecPrivateMode::BracketedPaste => {
                if self.state.bracketed_paste { 1 } else { 2 }
            }
            DecPrivateMode::ApplicationCursorKeys => {
                if self.state.application_cursor_keys { 1 } else { 2 }
            }
            DecPrivateMode::ShowCursor => {
                if self.state.show_cursor { 1 } else { 2 }
            }
            DecPrivateMode::CursorBlink => {
                if self.state.cursor_blink { 1 } else { 2 }
            }
            DecPrivateMode::MouseSGR => {
                if self.state.mouse_sgr { 1 } else { 2 }
            }
            DecPrivateMode::FocusEvents => {
                if self.state.focus_events { 1 } else { 2 }
            }
            DecPrivateMode::MouseTracking => {
                if self.state.mouse_tracking { 1 } else { 2 }
            }
            DecPrivateMode::MouseCellMotion => {
                if self.state.mouse_cell_motion { 1 } else { 2 }
            }
            DecPrivateMode::MouseAllMotion => {
                if self.state.mouse_all_motion { 1 } else { 2 }
            }
            DecPrivateMode::MouseUrxvt => {
                if self.state.mouse_urxvt { 1 } else { 2 }
            }
            DecPrivateMode::SynchronizedOutput => {
                if self.state.synchronized_output { 1 } else { 2 }
            }
            _ => 0, // Not recognized/implemented
        };

        // Format: ESC[?{mode};{value}$y
        let response = format!("\x1b[?{};{}$y", mode_num, value);
        self.pending_responses.push(response.into_bytes());
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
            DecPrivateMode::AutoWrapMode => {
                // Enable automatic line wrapping at right margin
                self.state.auto_wrap = true;
            }
            DecPrivateMode::BracketedPaste => {
                // Enable bracketed paste mode
                self.state.bracketed_paste = true;
            }
            DecPrivateMode::ApplicationCursorKeys => {
                // Enable application cursor keys mode
                self.state.application_cursor_keys = true;
            }
            DecPrivateMode::ShowCursor => {
                // Show cursor
                self.state.show_cursor = true;
            }
            DecPrivateMode::CursorBlink => {
                // Enable cursor blinking
                self.state.cursor_blink = true;
            }
            DecPrivateMode::MouseSGR => {
                // Enable SGR mouse tracking
                self.state.mouse_sgr = true;
            }
            DecPrivateMode::FocusEvents => {
                // Enable focus event reporting
                self.state.focus_events = true;
            }
            DecPrivateMode::MouseTracking => {
                // Enable mouse button event reporting
                self.state.mouse_tracking = true;
            }
            DecPrivateMode::MouseCellMotion => {
                // Enable mouse button + drag reporting
                self.state.mouse_cell_motion = true;
            }
            DecPrivateMode::MouseAllMotion => {
                // Enable mouse all motion reporting
                self.state.mouse_all_motion = true;
            }
            DecPrivateMode::MouseUrxvt => {
                // Enable urxvt-style mouse reporting
                self.state.mouse_urxvt = true;
            }
            DecPrivateMode::SynchronizedOutput => {
                // Enable synchronized output mode
                self.state.synchronized_output = true;
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
            DecPrivateMode::AutoWrapMode => {
                // Disable automatic line wrapping
                self.state.auto_wrap = false;
            }
            DecPrivateMode::BracketedPaste => {
                // Disable bracketed paste mode
                self.state.bracketed_paste = false;
            }
            DecPrivateMode::ApplicationCursorKeys => {
                // Disable application cursor keys mode
                self.state.application_cursor_keys = false;
            }
            DecPrivateMode::ShowCursor => {
                // Hide cursor
                self.state.show_cursor = false;
            }
            DecPrivateMode::CursorBlink => {
                // Disable cursor blinking
                self.state.cursor_blink = false;
            }
            DecPrivateMode::MouseSGR => {
                // Disable SGR mouse tracking
                self.state.mouse_sgr = false;
            }
            DecPrivateMode::FocusEvents => {
                // Disable focus event reporting
                self.state.focus_events = false;
            }
            DecPrivateMode::MouseTracking => {
                // Disable mouse button event reporting
                self.state.mouse_tracking = false;
            }
            DecPrivateMode::MouseCellMotion => {
                // Disable mouse button + drag reporting
                self.state.mouse_cell_motion = false;
            }
            DecPrivateMode::MouseAllMotion => {
                // Disable mouse all motion reporting
                self.state.mouse_all_motion = false;
            }
            DecPrivateMode::MouseUrxvt => {
                // Disable urxvt-style mouse reporting
                self.state.mouse_urxvt = false;
            }
            DecPrivateMode::SynchronizedOutput => {
                // Disable synchronized output mode
                self.state.synchronized_output = false;
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
                    self.state.reverse = false;
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
                    Self::handle_extended_color(
                        &mut iter,
                        true,
                        &mut self.state.fg,
                        &mut self.state.bg,
                    );
                }
                SgrParameter::ExtendedBackground => {
                    Self::handle_extended_color(
                        &mut iter,
                        false,
                        &mut self.state.fg,
                        &mut self.state.bg,
                    );
                }
                SgrParameter::ReverseVideo => {
                    self.state.reverse = true;
                }
                SgrParameter::NotReversed => {
                    self.state.reverse = false;
                }
                SgrParameter::DefaultUnderlineColor => {
                    // Default underline color - no-op (not yet implemented)
                    // We don't support separate underline colors yet
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
        // Swap colors if reverse video is enabled
        let (fg, bg) = if self.state.reverse {
            (self.state.bg, self.state.fg)
        } else {
            (self.state.fg, self.state.bg)
        };

        // Create cell with current attributes
        let cell = Cell {
            ch: c,
            fg,
            bg,
            bold: self.state.bold,
            italic: self.state.italic,
            underline: self.state.underline,
            reverse: self.state.reverse,
        };

        // Check if we need to wrap to next line
        if self.state.cursor.col >= self.state.grid.width {
            if self.state.auto_wrap {
                // Wrap to next line
                self.state.cursor.col = 0;
                self.state.cursor.row += 1;

                // If at bottom, scroll the viewport down
                if self.state.cursor.row >= self.state.grid.viewport_height {
                    self.state.cursor.row = self.state.grid.viewport_height - 1;
                    // TODO: Actual scrolling logic
                }
            } else {
                // No wrap: stay at right edge (overwrite last position)
                self.state.cursor.col = self.state.grid.width - 1;
            }
        }

        // Put the cell at cursor position
        self.state
            .grid
            .put_cell(cell, self.state.cursor.row, self.state.cursor.col);

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
        let has_gt = intermediates.first() == Some(&b'>');

        // Handle secondary DA (ESC[>c) specially
        if has_gt && action == 'c' {
            // Secondary DA - report terminal type and version
            // Format: ESC[>Pp;Pv;Pcc
            // Pp = terminal type (0 = VT100, 1 = VT220, etc.)
            // Pv = firmware version
            // Pc = ROM cartridge registration number
            // Report as VT220-compatible terminal
            let response = b"\x1b[>1;0;0c".to_vec();
            self.pending_responses.push(response);
            return;
        }

        if is_dec_private {
            // Handle DEC private modes
            match action {
                'h' => self.handle_dec_mode_set(params),
                'l' => self.handle_dec_mode_reset(params),
                'p' => {
                    // DECRQM (Request Mode) - query mode status
                    let mode_num = self.param_or(params, 0, 0);
                    self.generate_decrqm_response(mode_num);
                }
                'u' => {
                    // Unknown DEC private action 'u' - recognized, no-op
                }
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
                self.state.cursor.col =
                    (col.saturating_sub(1) as usize).min(self.state.grid.width - 1);
            }

            // Cursor movement
            CsiCommand::CursorUp { n } => {
                self.state.cursor.row = self.state.cursor.row.saturating_sub(n as usize);
            }

            CsiCommand::CursorDown { n } => {
                self.state.cursor.row =
                    (self.state.cursor.row + n as usize).min(self.state.grid.viewport_height - 1);
            }

            CsiCommand::CursorForward { n } => {
                self.state.cursor.col =
                    (self.state.cursor.col + n as usize).min(self.state.grid.width - 1);
            }

            CsiCommand::CursorBack { n } => {
                self.state.cursor.col = self.state.cursor.col.saturating_sub(n as usize);
            }

            CsiCommand::CursorHorizontalAbsolute { col } => {
                self.state.cursor.col =
                    (col.saturating_sub(1) as usize).min(self.state.grid.width - 1);
            }

            // Erase operations
            CsiCommand::EraseInDisplay { mode } => match mode {
                EraseMode::ToEnd => {
                    // Clear from cursor to end of current line
                    for col in self.state.cursor.col..self.state.grid.width {
                        self.state
                            .grid
                            .put_cell(Cell::default(), self.state.cursor.row, col);
                    }
                    // Clear all lines below cursor to end of viewport
                    let viewport_end =
                        self.state.grid.viewport_start + self.state.grid.viewport_height;
                    for row in (self.state.cursor.row + 1)..viewport_end {
                        self.state.grid.clear_line(row);
                    }
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
                        self.state
                            .grid
                            .put_cell(Cell::default(), self.state.cursor.row, col);
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
                        self.state
                            .grid
                            .put_cell(Cell::default(), self.state.cursor.row, col);
                    }
                }
                EraseMode::All => {
                    self.state.grid.clear_line(self.state.cursor.row);
                }
                EraseMode::ToBeginning => {
                    for col in 0..=self.state.cursor.col {
                        self.state
                            .grid
                            .put_cell(Cell::default(), self.state.cursor.row, col);
                    }
                }
                EraseMode::AllWithScrollback => {
                    // Not applicable to EraseInLine
                }
            },

            // Scrolling region (DECSTBM)
            CsiCommand::SetScrollingRegion { top, bottom } => {
                // Convert from 1-indexed to 0-indexed
                // top=0, bottom=0 means reset to full screen
                if top == 0 && bottom == 0 {
                    self.state.grid.reset_scroll_region();
                } else if top > 0 && bottom > 0 {
                    let top_idx = (top as usize).saturating_sub(1);
                    let bottom_idx = (bottom as usize).saturating_sub(1);
                    self.state.grid.set_scroll_region(top_idx, bottom_idx);
                }
                // Move cursor to home position (required by VT100 spec)
                self.state.cursor.row = self.state.grid.viewport_start;
                self.state.cursor.col = 0;
            }

            // Line manipulation (IL/DL)
            CsiCommand::InsertLines { n } => {
                // Insert n blank lines at cursor position within scrolling region
                let count = n.max(1) as usize;
                let cursor_row = self.state.cursor.row - self.state.grid.viewport_start;
                self.state.grid.insert_lines(cursor_row, count);
            }

            CsiCommand::DeleteLines { n } => {
                // Delete n lines at cursor position within scrolling region
                let count = n.max(1) as usize;
                let cursor_row = self.state.cursor.row - self.state.grid.viewport_start;
                self.state.grid.delete_lines(cursor_row, count);
            }

            // Already handled above
            CsiCommand::SelectGraphicRendition => {}

            // Device queries and window manipulation
            CsiCommand::DeviceStatusReport { n } => {
                // DSR - Device Status Report
                match n {
                    6 => {
                        // CPR - Cursor Position Report
                        // Report cursor position as ESC[{row};{col}R
                        let row = self.state.cursor.row + 1; // 1-based
                        let col = self.state.cursor.col + 1; // 1-based
                        let response = format!("\x1b[{};{}R", row, col);
                        self.pending_responses.push(response.into_bytes());
                    }
                    _ => {
                        // Other DSR queries not implemented
                    }
                }
            }

            CsiCommand::DeviceAttributes { n } => {
                // DA - Device Attributes
                if n == 0 {
                    // Primary DA - identify as VT100
                    // ESC[?1;2c = VT100 with Advanced Video Option
                    let response = b"\x1b[?1;2c".to_vec();
                    self.pending_responses.push(response);
                }
                // Secondary DA and others not implemented
            }

            CsiCommand::WindowManipulation { .. } => {
                // Window operations (resize, minimize, etc.)
                // Not implementable at core level
            }

            CsiCommand::SetCursorStyle { style } => {
                // Set cursor style (block, underline, bar)
                // DECSCUSR: 0=default(block), 1=block blink, 2=block steady,
                //           3=underline blink, 4=underline steady, 5=bar blink, 6=bar steady
                use crate::terminal::cursor::CursorStyle;

                let new_style = match style {
                    0 | 1 | 2 => CursorStyle::Block,
                    3 | 4 => CursorStyle::Underline,
                    5 | 6 => CursorStyle::Bar,
                    _ => CursorStyle::Block, // Unknown values default to block
                };

                self.state.cursor.style = new_style;

                // Odd parameters enable blinking, even disable it
                // Note: Blink state is controlled by cursor_blink field
                if style > 0 {
                    self.state.cursor_blink = style % 2 == 1;
                }
            }

            CsiCommand::VerticalPositionAbsolute { row } => {
                // Move cursor to absolute row, column unchanged
                self.state.cursor.row =
                    (row.saturating_sub(1) as usize).min(self.state.grid.viewport_height - 1);
            }

            CsiCommand::EraseCharacter { n } => {
                // Erase n characters at cursor position
                let start_col = self.state.cursor.col;
                let end_col = (start_col + n as usize).min(self.state.grid.width);
                for col in start_col..end_col {
                    self.state
                        .grid
                        .put_cell(Cell::default(), self.state.cursor.row, col);
                }
            }

            CsiCommand::ScrollDown { n } => {
                // Scroll viewport down by n lines (insert blank lines at top)
                let viewport_start = self.state.grid.viewport_start;
                for _ in 0..n {
                    let blank_row = vec![Cell::default(); self.state.grid.width];
                    self.state.grid.cells.insert(viewport_start, blank_row);
                }

                // Enforce scrollback limit
                if self.state.grid.cells.len() > self.state.grid.max_scrollback {
                    let excess = self.state.grid.cells.len() - self.state.grid.max_scrollback;
                    self.state.grid.cells.drain(0..excess);
                    self.state.grid.viewport_start =
                        self.state.grid.viewport_start.saturating_sub(excess);
                }
            }

            CsiCommand::ScrollUp { n } => {
                // Scroll viewport up by n lines (remove lines from top, add blank at bottom)
                let viewport_start = self.state.grid.viewport_start;

                // Remove n lines from viewport_start
                let lines_to_remove = (n as usize).min(self.state.grid.viewport_height);
                if viewport_start + lines_to_remove <= self.state.grid.cells.len() {
                    self.state
                        .grid
                        .cells
                        .drain(viewport_start..viewport_start + lines_to_remove);

                    // Add blank lines at the end of viewport
                    for _ in 0..lines_to_remove {
                        let blank_row = vec![Cell::default(); self.state.grid.width];
                        let insert_pos = (viewport_start + self.state.grid.viewport_height
                            - lines_to_remove)
                            .min(self.state.grid.cells.len());
                        self.state.grid.cells.insert(insert_pos, blank_row);
                    }
                }
            }

            CsiCommand::DeleteCharacter { n } => {
                // Delete n characters at cursor, shifting remaining chars left
                let row = self.state.cursor.row;
                let start_col = self.state.cursor.col;
                let width = self.state.grid.width;

                if start_col < width {
                    // Get the current row
                    let viewport_start = self.state.grid.viewport_start;
                    let absolute_row = viewport_start + row;

                    // Ensure row exists
                    while absolute_row >= self.state.grid.cells.len() {
                        self.state.grid.cells.push(vec![Cell::default(); width]);
                    }

                    let n_chars = (n as usize).min(width - start_col);

                    // Shift characters left by removing n chars at cursor position
                    for _ in 0..n_chars {
                        if start_col < self.state.grid.cells[absolute_row].len() {
                            self.state.grid.cells[absolute_row].remove(start_col);
                        }
                    }

                    // Add blank cells at the end to maintain width
                    while self.state.grid.cells[absolute_row].len() < width {
                        self.state.grid.cells[absolute_row].push(Cell::default());
                    }
                }
            }

            CsiCommand::ResetMode { mode: _ } => {
                // No-op: mode state tracking not yet implemented
                // Common modes: 4 (Insert Mode), 20 (Automatic Newline)
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

    #[test]
    fn test_reverse_video_enables() {
        let mut terminal = Terminal::new(80, 24);

        // Enable reverse video with ESC[7m
        terminal.process_bytes(b"\x1b[7m");

        // Reverse flag should be set
        assert!(terminal.state().reverse);
    }

    #[test]
    fn test_reverse_video_disables() {
        let mut terminal = Terminal::new(80, 24);

        // Enable then disable reverse video
        terminal.process_bytes(b"\x1b[7m");
        assert!(terminal.state().reverse);

        terminal.process_bytes(b"\x1b[27m");
        assert!(!terminal.state().reverse);
    }

    #[test]
    fn test_reverse_video_swaps_colors() {
        let mut terminal = Terminal::new(80, 24);

        // Set red foreground (ANSI 31) and blue background (ANSI 44)
        terminal.process_bytes(b"\x1b[31;44m");

        // Store original colors
        let orig_fg = terminal.state().fg;
        let orig_bg = terminal.state().bg;

        // Enable reverse and print character
        terminal.process_bytes(b"\x1b[7mX");

        // Check cell has swapped colors
        let viewport = terminal.state().grid.get_viewport();
        let cell = &viewport[0][0];

        assert_eq!(cell.ch, 'X');
        assert!(cell.reverse);
        // fg should be original bg, bg should be original fg
        assert_eq!(cell.fg.r, orig_bg.r);
        assert_eq!(cell.fg.g, orig_bg.g);
        assert_eq!(cell.fg.b, orig_bg.b);
        assert_eq!(cell.bg.r, orig_fg.r);
        assert_eq!(cell.bg.g, orig_fg.g);
        assert_eq!(cell.bg.b, orig_fg.b);
    }

    #[test]
    fn test_reverse_video_reset() {
        let mut terminal = Terminal::new(80, 24);

        // Enable reverse video
        terminal.process_bytes(b"\x1b[7m");
        assert!(terminal.state().reverse);

        // Reset with ESC[0m
        terminal.process_bytes(b"\x1b[0m");

        // Reverse should be cleared
        assert!(!terminal.state().reverse);
    }

    #[test]
    fn test_reverse_with_default_colors() {
        let mut terminal = Terminal::new(80, 24);

        // Default colors: white fg, black bg
        let default_fg = terminal.state().fg;
        let default_bg = terminal.state().bg;

        // Enable reverse and print
        terminal.process_bytes(b"\x1b[7mA");

        let viewport = terminal.state().grid.get_viewport();
        let cell = &viewport[0][0];

        // Colors should be swapped
        assert_eq!(cell.fg.r, default_bg.r);
        assert_eq!(cell.fg.g, default_bg.g);
        assert_eq!(cell.fg.b, default_bg.b);
        assert_eq!(cell.bg.r, default_fg.r);
        assert_eq!(cell.bg.g, default_fg.g);
        assert_eq!(cell.bg.b, default_fg.b);
    }

    #[test]
    fn test_multiple_reverse_toggles() {
        let mut terminal = Terminal::new(80, 24);

        // Toggle sequence: normal, reverse, normal, reverse
        terminal.process_bytes(b"N\x1b[7mR\x1b[27mN\x1b[7mR");

        let viewport = terminal.state().grid.get_viewport();

        // First 'N' should have reverse=false
        assert_eq!(viewport[0][0].ch, 'N');
        assert!(!viewport[0][0].reverse);

        // First 'R' should have reverse=true
        assert_eq!(viewport[0][1].ch, 'R');
        assert!(viewport[0][1].reverse);

        // Second 'N' should have reverse=false
        assert_eq!(viewport[0][2].ch, 'N');
        assert!(!viewport[0][2].reverse);

        // Second 'R' should have reverse=true
        assert_eq!(viewport[0][3].ch, 'R');
        assert!(viewport[0][3].reverse);
    }

    #[test]
    fn test_reverse_preserves_other_attributes() {
        let mut terminal = Terminal::new(80, 24);

        // Set bold, italic, red foreground, then enable reverse
        terminal.process_bytes(b"\x1b[1;3;31;7mX");

        let viewport = terminal.state().grid.get_viewport();
        let cell = &viewport[0][0];

        // All attributes should be set
        assert!(cell.bold);
        assert!(cell.italic);
        assert!(cell.reverse);
    }

    #[test]
    fn test_vpa_basic() {
        let mut terminal = Terminal::new(80, 24);
        terminal.state_mut().cursor.col = 10;

        // Move to row 15 (1-indexed -> 14 in 0-indexed)
        terminal.process_bytes(b"\x1b[15d");

        assert_eq!(terminal.state().cursor.row, 14);
        assert_eq!(terminal.state().cursor.col, 10); // Column unchanged
    }

    #[test]
    fn test_vpa_default() {
        let mut terminal = Terminal::new(80, 24);
        terminal.state_mut().cursor.row = 10;
        terminal.state_mut().cursor.col = 20;

        // VPA with no parameter defaults to row 1
        terminal.process_bytes(b"\x1b[d");

        assert_eq!(terminal.state().cursor.row, 0); // Row 1 -> index 0
        assert_eq!(terminal.state().cursor.col, 20); // Column unchanged
    }

    #[test]
    fn test_vpa_bounds() {
        let mut terminal = Terminal::new(80, 24);

        // Try to move beyond viewport height
        terminal.process_bytes(b"\x1b[100d");

        assert_eq!(terminal.state().cursor.row, 23); // Clamped to bottom (0-indexed)
    }

    #[test]
    fn test_ech_basic() {
        let mut terminal = Terminal::new(80, 24);

        // Write some characters
        terminal.process_bytes(b"ABCDEFGH");

        // Move cursor back to C
        terminal.state_mut().cursor.col = 2;

        // Erase 3 characters (CDE)
        terminal.process_bytes(b"\x1b[3X");

        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'A');
        assert_eq!(viewport[0][1].ch, 'B');
        assert_eq!(viewport[0][2].ch, ' '); // Erased
        assert_eq!(viewport[0][3].ch, ' '); // Erased
        assert_eq!(viewport[0][4].ch, ' '); // Erased
        assert_eq!(viewport[0][5].ch, 'F'); // Not erased
    }

    #[test]
    fn test_ech_default() {
        let mut terminal = Terminal::new(80, 24);

        terminal.process_bytes(b"HELLO");
        terminal.state_mut().cursor.col = 1;

        // ECH with no parameter erases 1 character
        terminal.process_bytes(b"\x1b[X");

        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'H');
        assert_eq!(viewport[0][1].ch, ' '); // Erased
        assert_eq!(viewport[0][2].ch, 'L');
    }

    #[test]
    fn test_ech_bounds() {
        let mut terminal = Terminal::new(5, 24); // Narrow terminal

        terminal.process_bytes(b"HELLO");
        terminal.state_mut().cursor.col = 3;

        // Try to erase 5 characters (should clamp to line width)
        terminal.process_bytes(b"\x1b[5X");

        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'H');
        assert_eq!(viewport[0][3].ch, ' '); // Erased
        assert_eq!(viewport[0][4].ch, ' '); // Erased
    }

    #[test]
    fn test_scroll_down_basic() {
        let mut terminal = Terminal::new(80, 24);

        // Write identifying text on first few lines
        terminal.process_bytes(b"Line1\n\rLine2\n\rLine3");

        // Scroll down by 2
        terminal.process_bytes(b"\x1b[2T");

        let viewport = terminal.state().grid.get_viewport();
        // First 2 lines should be blank (scrolled down)
        assert_eq!(viewport[0][0].ch, ' ');
        assert_eq!(viewport[1][0].ch, ' ');
        // Original line 1 should now be at row 2
        assert_eq!(viewport[2][0].ch, 'L');
        assert_eq!(viewport[2][1].ch, 'i');
    }

    #[test]
    fn test_scroll_down_default() {
        let mut terminal = Terminal::new(80, 24);

        terminal.process_bytes(b"Test");

        // SD with no parameter scrolls 1 line
        terminal.process_bytes(b"\x1b[T");

        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, ' '); // Blank line inserted
        assert_eq!(viewport[1][0].ch, 'T'); // Original content shifted down
    }

    #[test]
    fn test_auto_wrap_enabled() {
        let mut terminal = Terminal::new(5, 24); // Narrow terminal

        // Enable auto wrap (default, but explicit)
        terminal.process_bytes(b"\x1b[?7h");

        // Write more than width
        terminal.process_bytes(b"123456");

        let viewport = terminal.state().grid.get_viewport();
        // Should wrap to next line
        assert_eq!(viewport[0][4].ch, '5');
        assert_eq!(viewport[1][0].ch, '6');
    }

    #[test]
    fn test_auto_wrap_disabled() {
        let mut terminal = Terminal::new(5, 24);

        // Disable auto wrap
        terminal.process_bytes(b"\x1b[?7l");

        // Write more than width
        terminal.process_bytes(b"123456");

        let viewport = terminal.state().grid.get_viewport();
        // Should overwrite last position
        assert_eq!(viewport[0][4].ch, '6'); // Overwrote '5'
        assert_eq!(viewport[1][0].ch, ' '); // No wrap
    }

    #[test]
    fn test_auto_wrap_default() {
        let terminal = Terminal::new(80, 24);

        // Verify default is enabled (true)
        assert!(terminal.state().auto_wrap);
    }

    #[test]
    fn test_auto_wrap_toggle() {
        let mut terminal = Terminal::new(5, 24);

        // Default is enabled
        assert!(terminal.state().auto_wrap);

        // Disable
        terminal.process_bytes(b"\x1b[?7l");
        assert!(!terminal.state().auto_wrap);

        // Enable
        terminal.process_bytes(b"\x1b[?7h");
        assert!(terminal.state().auto_wrap);
    }

    #[test]
    fn test_scroll_up_basic() {
        let mut terminal = Terminal::new(80, 24);

        // Write multiple lines
        terminal.process_bytes(b"Line1\n\rLine2\n\rLine3\n\rLine4");

        // Scroll up by 2 (removes top 2 lines, adds blanks at bottom)
        terminal.process_bytes(b"\x1b[2S");

        let viewport = terminal.state().grid.get_viewport();
        // Line3 should now be at top (lines 1-2 removed)
        assert_eq!(viewport[0][0].ch, 'L');
        assert_eq!(viewport[0][4].ch, '3');
    }

    #[test]
    fn test_scroll_up_default() {
        let mut terminal = Terminal::new(80, 24);

        terminal.process_bytes(b"Line1\n\rLine2");

        // SU with no parameter scrolls 1 line
        terminal.process_bytes(b"\x1b[S");

        let viewport = terminal.state().grid.get_viewport();
        // Line2 should now be at top
        assert_eq!(viewport[0][0].ch, 'L');
        assert_eq!(viewport[0][4].ch, '2');
    }

    #[test]
    fn test_delete_character_basic() {
        let mut terminal = Terminal::new(80, 24);

        // Write some text
        terminal.process_bytes(b"ABCDEFGH");

        // Move cursor back to C (position 2)
        terminal.state_mut().cursor.col = 2;

        // Delete 3 characters (CDE) - FGH should shift left
        terminal.process_bytes(b"\x1b[3P");

        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'A');
        assert_eq!(viewport[0][1].ch, 'B');
        assert_eq!(viewport[0][2].ch, 'F'); // Shifted left from position 5
        assert_eq!(viewport[0][3].ch, 'G'); // Shifted left from position 6
        assert_eq!(viewport[0][4].ch, 'H'); // Shifted left from position 7
        assert_eq!(viewport[0][5].ch, ' '); // Blank fill
    }

    #[test]
    fn test_delete_character_default() {
        let mut terminal = Terminal::new(80, 24);

        terminal.process_bytes(b"HELLO");
        terminal.state_mut().cursor.col = 1;

        // DCH with no parameter deletes 1 character
        terminal.process_bytes(b"\x1b[P");

        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'H');
        assert_eq!(viewport[0][1].ch, 'L'); // Shifted left (was at position 2)
        assert_eq!(viewport[0][2].ch, 'L'); // Shifted left (was at position 3)
        assert_eq!(viewport[0][3].ch, 'O'); // Shifted left (was at position 4)
        assert_eq!(viewport[0][4].ch, ' '); // Blank fill
    }

    #[test]
    fn test_delete_character_vs_erase() {
        let mut terminal = Terminal::new(80, 24);

        // Test DCH (Delete Character - shifts left)
        terminal.process_bytes(b"ABCD");
        terminal.state_mut().cursor.col = 1;
        terminal.process_bytes(b"\x1b[1P"); // Delete 'B'

        {
            let viewport = terminal.state().grid.get_viewport();
            assert_eq!(viewport[0][0].ch, 'A');
            assert_eq!(viewport[0][1].ch, 'C'); // Shifted left
            assert_eq!(viewport[0][2].ch, 'D'); // Shifted left
        } // viewport dropped here

        // Compare with ECH (Erase Character - no shift)
        terminal.state_mut().cursor.row = 1;
        terminal.state_mut().cursor.col = 0;
        terminal.process_bytes(b"ABCD");
        terminal.state_mut().cursor.col = 1;
        terminal.process_bytes(b"\x1b[1X"); // Erase 'B'

        let viewport = terminal.state().grid.get_viewport(); // Fresh borrow
        assert_eq!(viewport[1][0].ch, 'A');
        assert_eq!(viewport[1][1].ch, ' '); // Erased, not shifted
        assert_eq!(viewport[1][2].ch, 'C'); // Stayed in place
        assert_eq!(viewport[1][3].ch, 'D'); // Stayed in place
    }

    #[test]
    fn test_erase_in_display_to_end() {
        let mut terminal = Terminal::new(80, 24);

        // Fill multiple lines with content
        terminal.process_bytes(b"Line 1\r\nLine 2\r\nLine 3\r\nLine 4");

        // Position cursor at column 3, row 1 (middle of "Line 2")
        terminal.state_mut().cursor.row = terminal.state().grid.viewport_start + 1;
        terminal.state_mut().cursor.col = 3;

        // ESC[J or ESC[0J - Erase from cursor to end of display
        terminal.process_bytes(b"\x1b[J");

        let viewport = terminal.state().grid.get_viewport();

        // Line 0 should be untouched
        assert_eq!(viewport[0][0].ch, 'L');
        assert_eq!(viewport[0][5].ch, '1');

        // Line 1 should be cleared from cursor position to end
        assert_eq!(viewport[1][0].ch, 'L'); // Before cursor
        assert_eq!(viewport[1][1].ch, 'i'); // Before cursor
        assert_eq!(viewport[1][2].ch, 'n'); // Before cursor
        assert_eq!(viewport[1][3].ch, ' '); // At cursor - cleared
        assert_eq!(viewport[1][4].ch, ' '); // After cursor - cleared
        assert_eq!(viewport[1][5].ch, ' '); // After cursor - cleared

        // Lines 2 and 3 should be completely cleared
        assert_eq!(viewport[2][0].ch, ' ');
        assert_eq!(viewport[2][5].ch, ' ');
        assert_eq!(viewport[3][0].ch, ' ');
        assert_eq!(viewport[3][5].ch, ' ');
    }

    #[test]
    fn test_erase_in_display_all() {
        let mut terminal = Terminal::new(80, 24);

        // Fill with content
        terminal.process_bytes(b"Line 1\r\nLine 2\r\nLine 3");

        // ESC[2J - Erase entire display
        terminal.process_bytes(b"\x1b[2J");

        let viewport = terminal.state().grid.get_viewport();

        // All lines should be cleared
        assert_eq!(viewport[0][0].ch, ' ');
        assert_eq!(viewport[1][0].ch, ' ');
        assert_eq!(viewport[2][0].ch, ' ');

        // Cursor should be at home position
        assert_eq!(terminal.state().cursor.row, 0);
        assert_eq!(terminal.state().cursor.col, 0);
    }

    #[test]
    fn test_reset_mode_basic() {
        let mut terminal = Terminal::new(80, 24);

        // ESC[4l - Reset Insert Mode
        terminal.process_bytes(b"\x1b[4l");

        // Should not panic or produce errors
        // Terminal should remain functional
        terminal.process_bytes(b"TEST");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'T');
    }

    #[test]
    fn test_reset_mode_default() {
        let mut terminal = Terminal::new(80, 24);

        // ESC[l - Reset with default mode 0
        terminal.process_bytes(b"\x1b[l");

        // Should parse without error
        terminal.process_bytes(b"OK");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'O');
    }

    #[test]
    fn test_reset_mode_multiple() {
        let mut terminal = Terminal::new(80, 24);

        // Test multiple reset mode sequences
        terminal.process_bytes(b"\x1b[4l"); // Insert Mode
        terminal.process_bytes(b"\x1b[20l"); // Automatic Newline

        // All should parse successfully
        terminal.process_bytes(b"PASS");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'P');
    }

    #[test]
    fn test_dec_application_cursor_keys_no_op() {
        let mut terminal = Terminal::new(80, 24);

        // Set and reset application cursor keys mode
        terminal.process_bytes(b"\x1b[?1h"); // Set
        terminal.process_bytes(b"\x1b[?1l"); // Reset

        // Should process silently without warnings
        // Terminal remains functional
        terminal.process_bytes(b"OK");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'O');
    }

    #[test]
    fn test_dec_show_cursor_no_op() {
        let mut terminal = Terminal::new(80, 24);

        // Set and reset show cursor mode
        terminal.process_bytes(b"\x1b[?25h"); // Show
        terminal.process_bytes(b"\x1b[?25l"); // Hide

        // Should process silently
        terminal.process_bytes(b"OK");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'O');
    }

    #[test]
    fn test_dec_bracketed_paste_no_op() {
        let mut terminal = Terminal::new(80, 24);

        // Set and reset bracketed paste mode
        terminal.process_bytes(b"\x1b[?2004h"); // Enable
        terminal.process_bytes(b"\x1b[?2004l"); // Disable

        // Should process silently
        terminal.process_bytes(b"OK");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'O');
    }

    #[test]
    fn test_dec_mouse_sgr_no_op() {
        let mut terminal = Terminal::new(80, 24);

        // Set and reset mouse SGR mode
        terminal.process_bytes(b"\x1b[?1006h"); // Enable
        terminal.process_bytes(b"\x1b[?1006l"); // Disable

        // Should process silently
        terminal.process_bytes(b"OK");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'O');
    }

    #[test]
    fn test_bracketed_paste_mode() {
        let mut terminal = Terminal::new(80, 24);

        // Initially false
        assert!(!terminal.state().bracketed_paste);

        // Enable bracketed paste
        terminal.process_bytes(b"\x1b[?2004h");
        assert!(terminal.state().bracketed_paste);

        // Disable bracketed paste
        terminal.process_bytes(b"\x1b[?2004l");
        assert!(!terminal.state().bracketed_paste);
    }

    #[test]
    fn test_application_cursor_keys_mode() {
        let mut terminal = Terminal::new(80, 24);

        // Initially false
        assert!(!terminal.state().application_cursor_keys);

        // Enable application cursor keys
        terminal.process_bytes(b"\x1b[?1h");
        assert!(terminal.state().application_cursor_keys);

        // Disable application cursor keys
        terminal.process_bytes(b"\x1b[?1l");
        assert!(!terminal.state().application_cursor_keys);
    }

    #[test]
    fn test_show_cursor_mode() {
        let mut terminal = Terminal::new(80, 24);

        // Initially true (cursor visible by default)
        assert!(terminal.state().show_cursor);

        // Hide cursor
        terminal.process_bytes(b"\x1b[?25l");
        assert!(!terminal.state().show_cursor);

        // Show cursor
        terminal.process_bytes(b"\x1b[?25h");
        assert!(terminal.state().show_cursor);
    }

    #[test]
    fn test_cursor_blink_mode() {
        let mut terminal = Terminal::new(80, 24);

        // Initially false (no blinking by default)
        assert!(!terminal.state().cursor_blink);

        // Enable cursor blinking
        terminal.process_bytes(b"\x1b[?12h");
        assert!(terminal.state().cursor_blink);

        // Disable cursor blinking
        terminal.process_bytes(b"\x1b[?12l");
        assert!(!terminal.state().cursor_blink);
    }

    #[test]
    fn test_mouse_sgr_mode() {
        let mut terminal = Terminal::new(80, 24);

        // Initially false
        assert!(!terminal.state().mouse_sgr);

        // Enable SGR mouse tracking
        terminal.process_bytes(b"\x1b[?1006h");
        assert!(terminal.state().mouse_sgr);

        // Disable SGR mouse tracking
        terminal.process_bytes(b"\x1b[?1006l");
        assert!(!terminal.state().mouse_sgr);
    }

    #[test]
    fn test_all_dec_modes_no_warnings() {
        let mut terminal = Terminal::new(80, 24);

        // Process all DEC modes that should be no-ops
        terminal.process_bytes(b"\x1b[?1h"); // ApplicationCursorKeys set
        terminal.process_bytes(b"\x1b[?1l"); // ApplicationCursorKeys reset
        terminal.process_bytes(b"\x1b[?25h"); // ShowCursor set
        terminal.process_bytes(b"\x1b[?25l"); // ShowCursor reset
        terminal.process_bytes(b"\x1b[?2004h"); // BracketedPaste set
        terminal.process_bytes(b"\x1b[?2004l"); // BracketedPaste reset
        terminal.process_bytes(b"\x1b[?1006h"); // MouseSGR set
        terminal.process_bytes(b"\x1b[?1006l"); // MouseSGR reset
        terminal.process_bytes(b"\x1b[?12h"); // CursorBlink set
        terminal.process_bytes(b"\x1b[?12l"); // CursorBlink reset

        // All should succeed without warnings
        terminal.process_bytes(b"PASS");
        let viewport = terminal.state().grid.get_viewport();
        assert_eq!(viewport[0][0].ch, 'P');
        assert_eq!(viewport[0][1].ch, 'A');
        assert_eq!(viewport[0][2].ch, 'S');
        assert_eq!(viewport[0][3].ch, 'S');
    }
}
