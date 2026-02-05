use anyhow::{Context as _, Result};
use winit::event_loop::EventLoop;

#[cfg(not(feature = "ui-gpu"))]
#[path = "main/cpu_impl.rs"]
mod cpu_impl;
#[cfg(not(feature = "ui-gpu"))]
use cpu_impl::App;

#[cfg(feature = "ui-gpu")]
#[path = "main/gpu_impl.rs"]
mod gpu_impl;
#[cfg(feature = "ui-gpu")]
use gpu_impl::App;

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
