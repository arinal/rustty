//! Application state and logic for terminal UI
//!
//! This module contains the shared application logic that works with any renderer
//! implementation (CPU or GPU).

use super::Renderer;
use std::sync::Arc;

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
    pub window: Option<Arc<winit::window::Window>>,
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
        let bytes = super::input::handle_keyboard_input(
            &mut self.base.session,
            key,
            text,
            &self.base.modifiers,
        );

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
        super::input::reset_cursor_blink(
            &mut self.base.cursor_visible_phase,
            &mut self.base.last_blink_toggle,
        );
    }

    /// Handle clipboard paste operation
    pub fn handle_paste(&mut self) {
        super::input::handle_paste(&mut self.base.session, &mut self.base.clipboard);
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
