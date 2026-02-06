use rustty::renderer::GpuRenderer;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

pub(crate) type AppInner = rustty::App<GpuRenderer>;

// Newtype wrapper to implement ApplicationHandler
pub(crate) struct App(pub AppInner);

impl App {
    pub fn new() -> Self {
        App(AppInner::new())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Helper macro to handle errors and exit on failure
        macro_rules! unwrap_or_die {
            ($result:expr, $msg:expr) => {
                match $result {
                    Ok(val) => val,
                    Err(e) => {
                        eprintln!("{}: {}", $msg, e);
                        event_loop.exit();
                        return;
                    }
                }
            };
        }

        if self.0.window.is_none() {
            println!("Creating window...");
            let window_attrs = Window::default_attributes()
                .with_title("Rustty Terminal (GPU)")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

            let window = Arc::new(unwrap_or_die!(
                event_loop.create_window(window_attrs),
                "Failed to create window"
            ));
            println!("Window created");

            // Initialize GPU renderer
            let renderer = unwrap_or_die!(
                pollster::block_on(GpuRenderer::new(window.clone())),
                "Failed to initialize GPU renderer"
            );
            println!("GPU renderer initialized");

            let size = window.inner_size();
            let (cols, rows) = {
                let (char_width, char_height) = renderer.char_dimensions();
                println!(
                    "Character dimensions: {}x{} pixels",
                    char_width, char_height
                );
                let cols = ((size.width as f32 - 20.0) / char_width).floor() as usize;
                let rows = ((size.height as f32 - 40.0) / char_height).floor() as usize;
                (cols.max(10), rows.max(3))
            };
            println!("Calculated grid size: {}x{}", cols, rows);

            self.0.window = Some(window);
            self.0.renderer = Some(renderer);
            self.0.base.session.resize(cols, rows);

            println!("Rendering initial frame...");
            if let Err(e) = self.0.render() {
                eprintln!("Initial render error: {}", e);
            }
            println!("Initial render complete");
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.0.process_shell_output() {
            eprintln!("Child process terminated, exiting...");
            event_loop.exit();
            return;
        }

        // Handle cursor blink animation
        if self.0.base.session.state().cursor_blink {
            let elapsed = self.0.base.last_blink_toggle.elapsed();
            if elapsed >= Duration::from_millis(530) {
                self.0.base.cursor_visible_phase = !self.0.base.cursor_visible_phase;
                self.0.base.last_blink_toggle = Instant::now();
                if let Some(window) = &self.0.window {
                    window.request_redraw();
                }
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.0.render() {
                    eprintln!("Render error: {}", e);
                }
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.0.base.modifiers = new_modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let text = event.text.as_ref().map(|s| s.as_str());
                    self.0.handle_keyboard_input(&event.logical_key, text);
                }
            }
            WindowEvent::Resized(new_size) => {
                let (cols, rows) = self.0.calculate_grid_size(new_size.width, new_size.height);
                println!(
                    "Window resized to: {}x{} -> grid: {}x{}",
                    new_size.width, new_size.height, cols, rows
                );
                self.0.base.session.resize(cols, rows);

                // Resize GPU surface
                if let Some(renderer) = &mut self.0.renderer {
                    if let Err(e) = renderer.resize(new_size.width, new_size.height) {
                        eprintln!("Failed to resize renderer: {}", e);
                    }
                }

                if let Some(window) = &self.0.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Focused(focused) => {
                self.0.handle_focus_event(focused);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button_code = match button {
                    winit::event::MouseButton::Left => 0,
                    winit::event::MouseButton::Middle => 1,
                    winit::event::MouseButton::Right => 2,
                    _ => return,
                };

                let pressed = state == ElementState::Pressed;

                self.0.handle_mouse_button(button_code, pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((col, row)) = self.0.window_to_grid_coords(position.x, position.y) {
                    self.0.base.last_mouse_position = Some((col, row));
                    self.0.handle_cursor_moved(col, row);
                }
            }
            _ => {}
        }
    }
}
