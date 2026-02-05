//! CPU-based renderer using Raqote and Softbuffer
//!
//! This module provides a software-based rendering backend that works on all platforms
//! without requiring GPU drivers.

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
                    let bg_rect = raqote::Path {
                        ops: vec![
                            raqote::PathOp::MoveTo(raqote::Point::new(x, y - 15.0)),
                            raqote::PathOp::LineTo(raqote::Point::new(
                                x + self.char_width,
                                y - 15.0,
                            )),
                            raqote::PathOp::LineTo(raqote::Point::new(
                                x + self.char_width,
                                y + 5.0,
                            )),
                            raqote::PathOp::LineTo(raqote::Point::new(x, y + 5.0)),
                            raqote::PathOp::Close,
                        ],
                        winding: raqote::Winding::NonZero,
                    };
                    dt.fill(
                        &bg_rect,
                        &Source::Solid(SolidSource::from_unpremultiplied_argb(
                            0xff, cell.bg.r, cell.bg.g, cell.bg.b,
                        )),
                        &raqote::DrawOptions::new(),
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
                            // Brighten by increasing each component (clamped to 255)
                            let brighten = |c: u8| -> u8 { ((c as u16 * 3 / 2).min(255)) as u8 };
                            r = brighten(r);
                            g = brighten(g);
                            b = brighten(b);
                        }

                        if cell.italic {
                            // Add cyan tint to distinguish italic text
                            g = ((g as u16 + 30).min(255)) as u8;
                            b = ((b as u16 + 30).min(255)) as u8;
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
                            let underline_y = y + 2.0;
                            let underline_path = raqote::Path {
                                ops: vec![
                                    raqote::PathOp::MoveTo(raqote::Point::new(x, underline_y)),
                                    raqote::PathOp::LineTo(raqote::Point::new(
                                        x + self.char_width,
                                        underline_y,
                                    )),
                                ],
                                winding: raqote::Winding::NonZero,
                            };
                            dt.stroke(
                                &underline_path,
                                &Source::Solid(SolidSource::from_unpremultiplied_argb(
                                    0xff, r, g, b,
                                )),
                                &raqote::StrokeStyle {
                                    width: 1.0,
                                    ..Default::default()
                                },
                                &raqote::DrawOptions::new(),
                            );
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
                    let cursor_rect = raqote::Path {
                        ops: vec![
                            raqote::PathOp::MoveTo(raqote::Point::new(cursor_x, cursor_y - 15.0)),
                            raqote::PathOp::LineTo(raqote::Point::new(
                                cursor_x + self.char_width,
                                cursor_y - 15.0,
                            )),
                            raqote::PathOp::LineTo(raqote::Point::new(
                                cursor_x + self.char_width,
                                cursor_y + 5.0,
                            )),
                            raqote::PathOp::LineTo(raqote::Point::new(cursor_x, cursor_y + 5.0)),
                            raqote::PathOp::Close,
                        ],
                        winding: raqote::Winding::NonZero,
                    };
                    dt.fill(
                        &cursor_rect,
                        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, 255, 255, 255)),
                        &raqote::DrawOptions::new(),
                    );
                }
                CursorStyle::Underline => {
                    let underline_y = cursor_y + 3.0;
                    let underline_path = raqote::Path {
                        ops: vec![
                            raqote::PathOp::MoveTo(raqote::Point::new(cursor_x, underline_y)),
                            raqote::PathOp::LineTo(raqote::Point::new(
                                cursor_x + self.char_width,
                                underline_y,
                            )),
                        ],
                        winding: raqote::Winding::NonZero,
                    };
                    dt.stroke(
                        &underline_path,
                        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, 255, 255, 255)),
                        &raqote::StrokeStyle {
                            width: 2.0,
                            ..Default::default()
                        },
                        &raqote::DrawOptions::new(),
                    );
                }
                CursorStyle::Bar => {
                    let bar_path = raqote::Path {
                        ops: vec![
                            raqote::PathOp::MoveTo(raqote::Point::new(cursor_x, cursor_y - 15.0)),
                            raqote::PathOp::LineTo(raqote::Point::new(cursor_x, cursor_y + 5.0)),
                        ],
                        winding: raqote::Winding::NonZero,
                    };
                    dt.stroke(
                        &bar_path,
                        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, 255, 255, 255)),
                        &raqote::StrokeStyle {
                            width: 2.0,
                            ..Default::default()
                        },
                        &raqote::DrawOptions::new(),
                    );
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
