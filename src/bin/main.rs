use anyhow::{Context as _, Result};
use winit::event_loop::EventLoop;

#[cfg(not(feature = "ui-gpu"))]
#[path = "ui/cpu_ui.rs"]
mod cpu_ui;
#[cfg(not(feature = "ui-gpu"))]
use cpu_ui::App;

#[cfg(feature = "ui-gpu")]
#[path = "ui/gpu_ui.rs"]
mod gpu_ui;
#[cfg(feature = "ui-gpu")]
use gpu_ui::App;

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
