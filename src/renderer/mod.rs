//! Renderer implementations for the Rustty terminal emulator
//!
//! This module provides different rendering backends (CPU and GPU) that can be
//! used to display terminal content. All renderers implement the `Renderer` trait
//! for uniform behavior.

pub mod app;
pub mod input;

#[cfg(feature = "ui-cpu")]
pub mod cpu;

#[cfg(feature = "ui-gpu")]
pub mod gpu;

// Re-export common types
pub use app::{App, AppBase};

// Re-export renderers for convenience
#[cfg(feature = "ui-cpu")]
pub use cpu::CpuRenderer;

#[cfg(feature = "ui-gpu")]
pub use gpu::GpuRenderer;

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
