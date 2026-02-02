use anyhow::{Context as _, Result};
use rustty::app::App;
use winit::event_loop::EventLoop;

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let mut app = App::new();

    println!("Running event loop...");
    event_loop.run_app(&mut app)?;

    Ok(())
}
