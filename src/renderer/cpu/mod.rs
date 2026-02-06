//! CPU-based renderer using Raqote and Softbuffer
//!
//! This module provides a software-based rendering backend that works on all platforms
//! without requiring GPU drivers.

mod drawing;

use anyhow::{Context as _, Result};
use raqote::{DrawTarget, SolidSource, Source};
use softbuffer::Surface;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

/// CPU renderer using Raqote for 2D graphics and Softbuffer for display
pub struct CpuRenderer {
    surface: Surface<Arc<Window>, Arc<Window>>,
    font: font_kit::font::Font,
    char_width: f32,
    char_height: f32,
    font_size: f32,
}

impl CpuRenderer {
    /// Create a new CPU renderer
    pub fn new(
        surface: Surface<Arc<Window>, Arc<Window>>,
        font: font_kit::font::Font,
        char_width: f32,
        char_height: f32,
        font_size: f32,
    ) -> Self {
        Self {
            surface,
            font,
            char_width,
            char_height,
            font_size,
        }
    }

    /// Render with custom cursor visibility
    ///
    /// This method allows the caller to control cursor visibility (e.g., for blinking).
    pub fn render_with_blink(
        &mut self,
        state: &crate::TerminalState,
        cursor_visible: bool,
    ) -> Result<()> {
        let size_width = 800; // Will get actual size from window
        let size_height = 600;

        let width = size_width as i32;
        let height = size_height as i32;

        let w = NonZeroU32::new(size_width).context("Window width is zero")?;
        let h = NonZeroU32::new(size_height).context("Window height is zero")?;

        self.surface
            .resize(w, h)
            .map_err(|e| anyhow::anyhow!("Failed to resize surface: {:?}", e))?;

        let mut dt = DrawTarget::new(width, height);
        dt.clear(SolidSource::from_unpremultiplied_argb(0xff, 0, 0, 0));

        let offset_x = 10.0;
        let offset_y = 20.0;

        let viewport = state.grid.get_viewport();
        for (row, line) in viewport.iter().enumerate() {
            for (col, cell) in line.iter().enumerate() {
                let x = offset_x + col as f32 * self.char_width;
                let y = offset_y + row as f32 * self.char_height;

                // Draw background
                if cell.bg.r != 0 || cell.bg.g != 0 || cell.bg.b != 0 {
                    drawing::draw_background(
                        &mut dt,
                        x,
                        y,
                        self.char_width,
                        cell.bg.r,
                        cell.bg.g,
                        cell.bg.b,
                    );
                }

                // Draw character
                if cell.ch != ' ' && !cell.ch.is_control() {
                    let text = cell.ch.to_string();
                    if self.font.glyph_for_char(cell.ch).is_some() {
                        // Apply bold and/or italic effects
                        let mut r = cell.fg.r;
                        let mut g = cell.fg.g;
                        let mut b = cell.fg.b;

                        if cell.bold {
                            (r, g, b) = drawing::apply_bold(r, g, b);
                        }

                        if cell.italic {
                            (r, g, b) = drawing::apply_italic(r, g, b);
                        }

                        dt.draw_text(
                            &self.font,
                            self.font_size,
                            &text,
                            raqote::Point::new(x, y),
                            &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, r, g, b)),
                            &raqote::DrawOptions::new(),
                        );

                        // Draw underline if needed
                        if cell.underline {
                            drawing::draw_underline(&mut dt, x, y, self.char_width, r, g, b);
                        }
                    }
                }
            }
        }

        // Draw cursor
        let cursor_viewport_row = state.cursor.row.saturating_sub(state.grid.viewport_start);

        if cursor_visible && cursor_viewport_row < state.grid.viewport_height {
            let cursor_x = offset_x + state.cursor.col as f32 * self.char_width;
            let cursor_y = offset_y + cursor_viewport_row as f32 * self.char_height;
            let cursor_style = state.cursor.style;

            use crate::CursorStyle;

            match cursor_style {
                CursorStyle::Block => {
                    drawing::draw_block_cursor(&mut dt, cursor_x, cursor_y, self.char_width);
                }
                CursorStyle::Underline => {
                    drawing::draw_underline_cursor(&mut dt, cursor_x, cursor_y, self.char_width);
                }
                CursorStyle::Bar => {
                    drawing::draw_bar_cursor(&mut dt, cursor_x, cursor_y);
                }
            }
        }

        let dt_data = dt.get_data();
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| anyhow::anyhow!("Failed to get buffer: {:?}", e))?;

        for (i, pixel) in dt_data.iter().enumerate() {
            if i < buffer.len() {
                buffer[i] = *pixel;
            }
        }

        buffer
            .present()
            .map_err(|e| anyhow::anyhow!("Failed to present buffer: {:?}", e))?;
        Ok(())
    }
}

impl super::Renderer for CpuRenderer {
    fn char_dimensions(&self) -> (f32, f32) {
        (self.char_width, self.char_height)
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let w = NonZeroU32::new(width).context("Window width is zero")?;
        let h = NonZeroU32::new(height).context("Window height is zero")?;
        self.surface
            .resize(w, h)
            .map_err(|e| anyhow::anyhow!("Failed to resize surface: {:?}", e))?;
        Ok(())
    }

    fn render(&mut self, state: &crate::TerminalState) -> Result<()> {
        // Default to visible cursor for trait method
        self.render_with_blink(state, true)
    }

    fn render_with_blink(
        &mut self,
        state: &crate::TerminalState,
        cursor_visible: bool,
    ) -> Result<()> {
        // Delegate to the public method
        CpuRenderer::render_with_blink(self, state, cursor_visible)
    }

    fn is_initialized(&self) -> bool {
        // CPU renderer is always initialized once created
        true
    }
}
