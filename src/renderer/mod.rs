//! Renderer implementations for the Rustty terminal emulator
//!
//! This module provides different rendering backends (CPU and GPU) that can be
//! used to display terminal content. All renderers implement the `Renderer` trait
//! for uniform behavior.

#[cfg(feature = "ui-cpu")]
pub mod cpu;

#[cfg(feature = "ui-gpu")]
pub mod gpu;

// Re-export renderers for convenience
#[cfg(feature = "ui-cpu")]
pub use cpu::CpuRenderer;

#[cfg(feature = "ui-gpu")]
pub use gpu::GpuRenderer;

/// Generate mouse event escape sequence
///
/// Converts mouse events to appropriate ANSI escape sequences based on the
/// active mouse tracking mode.
///
/// # Arguments
///
/// * `state` - Terminal state to check which mouse mode is active
/// * `button` - Mouse button (0=left, 1=middle, 2=right)
/// * `col` - Grid column (0-indexed)
/// * `row` - Grid row (0-indexed)
/// * `pressed` - true for press, false for release
///
/// # Returns
///
/// ANSI escape sequence bytes, or empty vec if no mouse mode is active
pub fn generate_mouse_sequence(
    state: &crate::TerminalState,
    button: u8,
    col: usize,
    row: usize,
    pressed: bool,
) -> Vec<u8> {
    // Convert button to protocol value (0=left, 1=middle, 2=right, 3=release)
    let cb = if !pressed {
        3 // Release
    } else {
        button
    };

    if state.mouse_sgr {
        // SGR mouse protocol: ESC[<Cb;Cx;CyM/m
        // M for press, m for release
        let suffix = if pressed { 'M' } else { 'm' };
        format!("\x1b[<{};{};{}{}", cb, col + 1, row + 1, suffix).into_bytes()
    } else if state.mouse_tracking || state.mouse_cell_motion {
        // X10/X11 mouse protocol: ESC[MCbCxCy
        // Coordinates are encoded as value + 32 + 1 (1-indexed)
        let encoded_button = (cb + 32) as u8;
        let encoded_col = (col + 1 + 32).min(255) as u8;
        let encoded_row = (row + 1 + 32).min(255) as u8;
        vec![0x1b, b'[', b'M', encoded_button, encoded_col, encoded_row]
    } else {
        vec![]
    }
}

/// Abstraction for different rendering backends (CPU, GPU)
///
/// This trait allows code to work with both CPU and GPU renderers uniformly,
/// enabling extraction of shared logic that depends on renderer capabilities.
pub trait Renderer {
    /// Get character cell dimensions in pixels
    ///
    /// Returns (width, height) tuple representing the size of each character cell.
    fn char_dimensions(&self) -> (f32, f32);

    /// Resize the renderer surface
    ///
    /// Called when the window is resized to update the rendering surface dimensions.
    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()>;

    /// Render the terminal state to the screen
    ///
    /// Takes the current terminal state and renders it to the display.
    fn render(&mut self, state: &crate::TerminalState) -> anyhow::Result<()>;

    /// Render with custom cursor visibility (for blinking support)
    ///
    /// This method allows the caller to control cursor visibility independently.
    fn render_with_blink(
        &mut self,
        state: &crate::TerminalState,
        cursor_visible: bool,
    ) -> anyhow::Result<()>;

    /// Check if renderer is initialized and ready to render
    ///
    /// Returns true if the renderer has been set up and can accept render calls.
    fn is_initialized(&self) -> bool;
}

/// Common application state shared between CPU and GPU renderers
///
/// This struct contains all the state that is identical between the two renderer
/// implementations, eliminating duplication and ensuring consistent behavior.
pub struct AppBase {
    /// Terminal session managing the shell process and terminal state
    pub session: crate::TerminalSession,
    /// Current keyboard modifier state (Shift, Ctrl, Alt, etc.)
    pub modifiers: winit::keyboard::ModifiersState,
    /// Current cursor blink phase (true = visible, false = hidden)
    pub cursor_visible_phase: bool,
    /// Last time the cursor blink state was toggled
    pub last_blink_toggle: std::time::Instant,
    /// System clipboard for copy/paste operations
    pub clipboard: Option<arboard::Clipboard>,
    /// Last mouse position in grid coordinates (col, row)
    pub last_mouse_position: Option<(usize, usize)>,
    /// Bitmask of currently pressed mouse buttons
    pub mouse_buttons_pressed: u8,
}

impl AppBase {
    /// Create a new AppBase with default values
    pub fn new(cols: usize, rows: usize) -> Result<Self, String> {
        let session = crate::TerminalSession::new(cols, rows)
            .map_err(|e| format!("Failed to create terminal session: {}", e))?;

        Ok(Self {
            session,
            modifiers: winit::keyboard::ModifiersState::empty(),
            cursor_visible_phase: true,
            last_blink_toggle: std::time::Instant::now(),
            clipboard: arboard::Clipboard::new().ok(),
            last_mouse_position: None,
            mouse_buttons_pressed: 0,
        })
    }

    /// Process shell output from the PTY
    ///
    /// Returns false if the shell process has exited.
    pub fn process_shell_output(&mut self) -> bool {
        self.session.process_output()
    }

    /// Calculate grid dimensions based on window size and character dimensions
    pub fn calculate_grid_size(
        window_width: u32,
        window_height: u32,
        char_width: f32,
        char_height: f32,
    ) -> (usize, usize) {
        let cols = ((window_width as f32 - 20.0) / char_width).floor() as usize;
        let rows = ((window_height as f32 - 40.0) / char_height).floor() as usize;
        (cols.max(10), rows.max(3))
    }

    /// Convert window coordinates to grid coordinates
    pub fn window_to_grid_coords(
        x: f64,
        y: f64,
        char_width: f32,
        char_height: f32,
    ) -> Option<(usize, usize)> {
        // Account for rendering offset (10px horizontal, 20px vertical)
        let grid_x = (x - 10.0) / char_width as f64;
        let grid_y = (y - 20.0) / char_height as f64;

        if grid_x >= 0.0 && grid_y >= 0.0 {
            Some((grid_x.floor() as usize, grid_y.floor() as usize))
        } else {
            None
        }
    }
}

/// Generic application structure for terminal UI
///
/// This struct provides common functionality for both CPU and GPU renderers,
/// reducing code duplication across binaries.
pub struct App<R: Renderer> {
    /// Common terminal state and clipboard
    pub base: AppBase,
    /// Window (Arc-wrapped for GPU compatibility)
    pub window: Option<std::sync::Arc<winit::window::Window>>,
    /// Renderer implementation (CPU or GPU)
    pub renderer: Option<R>,
}

impl<R: Renderer> Default for App<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Renderer> App<R> {
    /// Create a new App with default values
    pub fn new() -> Self {
        let base = AppBase::new(80, 24).expect("Failed to create AppBase");

        Self {
            base,
            window: None,
            renderer: None,
        }
    }

    /// Calculate grid dimensions based on window size
    pub fn calculate_grid_size(&self, window_width: u32, window_height: u32) -> (usize, usize) {
        if let Some(renderer) = &self.renderer {
            let (char_width, char_height) = renderer.char_dimensions();
            let cols = ((window_width as f32 - 20.0) / char_width).floor() as usize;
            let rows = ((window_height as f32 - 40.0) / char_height).floor() as usize;
            (cols.max(10), rows.max(3))
        } else {
            (80, 24) // Default fallback
        }
    }

    /// Process shell output from PTY and request redraw if needed
    pub fn process_shell_output(&mut self) -> bool {
        let still_running = self.base.process_shell_output();

        if let Some(window) = &self.window {
            window.request_redraw();
        }

        still_running
    }

    /// Render terminal state to screen
    pub fn render(&mut self) -> anyhow::Result<()> {
        use anyhow::Context;

        let renderer = self.renderer.as_mut().context("No renderer available")?;
        let state = self.base.session.state();

        // Calculate cursor visibility based on blink phase
        let cursor_visible =
            state.show_cursor && (!state.cursor_blink || self.base.cursor_visible_phase);

        // Delegate to renderer's render_with_blink method
        renderer.render_with_blink(state, cursor_visible)?;
        Ok(())
    }

    /// Handle keyboard input events
    pub fn handle_keyboard_input(&mut self, key: &winit::keyboard::Key, text: Option<&str>) {
        let bytes =
            input::handle_keyboard_input(&mut self.base.session, key, text, &self.base.modifiers);

        // None means Ctrl+V was pressed - handle paste
        if bytes.is_none() {
            return self.handle_paste();
        }

        if let Some(data) = bytes {
            if let Err(e) = self.base.session.write_input(&data) {
                eprintln!("Failed to write to shell: {}", e);
            }
        }

        // Reset cursor blink phase to visible on input
        input::reset_cursor_blink(
            &mut self.base.cursor_visible_phase,
            &mut self.base.last_blink_toggle,
        );
    }

    /// Handle clipboard paste operation
    pub fn handle_paste(&mut self) {
        input::handle_paste(&mut self.base.session, &mut self.base.clipboard);
    }

    /// Convert window coordinates to grid coordinates
    pub fn window_to_grid_coords(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if let Some(renderer) = &self.renderer {
            let (char_width, char_height) = renderer.char_dimensions();
            AppBase::window_to_grid_coords(x, y, char_width, char_height)
        } else {
            None
        }
    }
}

/// Input handling functions
pub mod input {
    use crate::TerminalSession;
    use arboard::Clipboard;
    use std::time::Instant;
    use winit::keyboard::{Key, ModifiersState, NamedKey};

    /// Handle keyboard input and generate appropriate sequences
    ///
    /// This function processes keyboard events and sends the appropriate
    /// escape sequences or characters to the terminal session.
    ///
    /// Returns None if paste was triggered (Ctrl+V), otherwise returns the bytes to send.
    pub fn handle_keyboard_input(
        session: &mut TerminalSession,
        key: &Key,
        text: Option<&str>,
        modifiers: &ModifiersState,
    ) -> Option<Vec<u8>> {
        match key {
            Key::Named(named) => match named {
                NamedKey::Enter => Some(b"\r".to_vec()),
                NamedKey::Backspace => Some(b"\x7f".to_vec()),
                NamedKey::Tab => Some(b"\t".to_vec()),
                NamedKey::Space => Some(b" ".to_vec()),
                NamedKey::Escape => Some(b"\x1b".to_vec()),
                NamedKey::ArrowUp => {
                    if session.state().application_cursor_keys {
                        Some(b"\x1bOA".to_vec())
                    } else {
                        Some(b"\x1b[A".to_vec())
                    }
                }
                NamedKey::ArrowDown => {
                    if session.state().application_cursor_keys {
                        Some(b"\x1bOB".to_vec())
                    } else {
                        Some(b"\x1b[B".to_vec())
                    }
                }
                NamedKey::ArrowRight => {
                    if session.state().application_cursor_keys {
                        Some(b"\x1bOC".to_vec())
                    } else {
                        Some(b"\x1b[C".to_vec())
                    }
                }
                NamedKey::ArrowLeft => {
                    if session.state().application_cursor_keys {
                        Some(b"\x1bOD".to_vec())
                    } else {
                        Some(b"\x1b[D".to_vec())
                    }
                }
                NamedKey::Home => Some(b"\x1b[H".to_vec()),
                NamedKey::End => Some(b"\x1b[F".to_vec()),
                NamedKey::PageUp => Some(b"\x1b[5~".to_vec()),
                NamedKey::PageDown => Some(b"\x1b[6~".to_vec()),
                NamedKey::Delete => Some(b"\x1b[3~".to_vec()),
                NamedKey::Insert => Some(b"\x1b[2~".to_vec()),
                _ => None,
            },
            Key::Character(s) => {
                let chars: Vec<char> = s.chars().collect();
                if chars.len() == 1 {
                    let ch = chars[0];

                    // Check if Ctrl modifier is pressed
                    if modifiers.control_key() && ch.is_ascii_alphabetic() {
                        let lower = ch.to_ascii_lowercase();

                        // Intercept Ctrl+V for paste - return None as signal
                        if lower == 'v' {
                            return None;
                        }

                        // Ctrl+letter produces control codes 1-26
                        let ctrl_code = (lower as u8) - b'a' + 1;
                        Some(vec![ctrl_code])
                    } else if let Some(text_str) = text {
                        Some(text_str.as_bytes().to_vec())
                    } else {
                        Some(s.as_bytes().to_vec())
                    }
                } else if let Some(text_str) = text {
                    Some(text_str.as_bytes().to_vec())
                } else {
                    Some(s.as_bytes().to_vec())
                }
            }
            _ => None,
        }
    }

    /// Handle clipboard paste operation
    ///
    /// Reads text from the clipboard and sends it to the terminal,
    /// optionally wrapping it with bracketed paste sequences.
    pub fn handle_paste(session: &mut TerminalSession, clipboard: &mut Option<Clipboard>) {
        if let Some(clipboard) = clipboard {
            match clipboard.get_text() {
                Ok(text) => {
                    let data = if session.state().bracketed_paste {
                        // Wrap pasted text with bracketed paste sequences
                        let mut result = Vec::new();
                        result.extend_from_slice(b"\x1b[200~");
                        result.extend_from_slice(text.as_bytes());
                        result.extend_from_slice(b"\x1b[201~");
                        result
                    } else {
                        text.as_bytes().to_vec()
                    };

                    if let Err(e) = session.write_input(&data) {
                        eprintln!("Failed to write paste: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read clipboard: {}", e);
                }
            }
        }
    }

    /// Reset cursor blink state to visible
    pub fn reset_cursor_blink(cursor_visible_phase: &mut bool, last_blink_toggle: &mut Instant) {
        *cursor_visible_phase = true;
        *last_blink_toggle = Instant::now();
    }

    /// Handle focus events (focus in/out)
    pub fn handle_focus_event(session: &mut TerminalSession, focused: bool) {
        if session.state().focus_events {
            let sequence = if focused { b"\x1b[I" } else { b"\x1b[O" };
            if let Err(e) = session.write_input(sequence) {
                eprintln!("Failed to write focus event: {}", e);
            }
        }
    }

    /// Handle mouse button press/release events
    pub fn handle_mouse_button(
        session: &mut TerminalSession,
        button_code: u8,
        pressed: bool,
        mouse_buttons_pressed: &mut u8,
        last_mouse_position: Option<(usize, usize)>,
    ) -> bool {
        if pressed {
            *mouse_buttons_pressed |= 1 << button_code;
        } else {
            *mouse_buttons_pressed &= !(1 << button_code);
        }

        if let Some((col, row)) = last_mouse_position {
            let term_state = session.state();
            if term_state.mouse_tracking || term_state.mouse_cell_motion || term_state.mouse_sgr {
                let sequence =
                    super::generate_mouse_sequence(term_state, button_code, col, row, pressed);
                if !sequence.is_empty() {
                    if let Err(e) = session.write_input(&sequence) {
                        eprintln!("Failed to write mouse event: {}", e);
                        return false;
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Handle cursor moved events for mouse tracking
    pub fn handle_cursor_moved(
        session: &mut TerminalSession,
        col: usize,
        row: usize,
        prev_position: Option<(usize, usize)>,
        mouse_buttons_pressed: u8,
    ) -> bool {
        let term_state = session.state();
        if term_state.mouse_cell_motion || term_state.mouse_sgr {
            if mouse_buttons_pressed != 0 && prev_position != Some((col, row)) {
                let button_code = mouse_buttons_pressed.trailing_zeros() as u8;
                let sequence =
                    super::generate_mouse_sequence(term_state, button_code, col, row, true);
                if !sequence.is_empty() {
                    if let Err(e) = session.write_input(&sequence) {
                        eprintln!("Failed to write mouse motion: {}", e);
                        return false;
                    }
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mouse_sequence_sgr_press() {
        let mut state = crate::TerminalState::new(80, 24);
        state.mouse_sgr = true;

        let seq = generate_mouse_sequence(&state, 0, 5, 10, true);
        assert_eq!(seq, b"\x1b[<0;6;11M");
    }

    #[test]
    fn test_generate_mouse_sequence_sgr_release() {
        let mut state = crate::TerminalState::new(80, 24);
        state.mouse_sgr = true;

        let seq = generate_mouse_sequence(&state, 0, 5, 10, false);
        assert_eq!(seq, b"\x1b[<3;6;11m");
    }

    #[test]
    fn test_generate_mouse_sequence_x11() {
        let mut state = crate::TerminalState::new(80, 24);
        state.mouse_tracking = true;

        let seq = generate_mouse_sequence(&state, 0, 5, 10, true);
        assert_eq!(seq, vec![0x1b, b'[', b'M', 32, 38, 43]);
    }

    #[test]
    fn test_generate_mouse_sequence_no_mode() {
        let state = crate::TerminalState::new(80, 24);

        let seq = generate_mouse_sequence(&state, 0, 5, 10, true);
        assert_eq!(seq, Vec::<u8>::new());
    }
}
