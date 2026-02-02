use anyhow::{Context as _, Result};
use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use raqote::{DrawTarget, SolidSource, Source};
use rustty::terminal::{Shell, Terminal};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub struct App {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    terminal: Terminal,
    pub font: Option<font_kit::font::Font>,
    shell: Option<Shell>,
    // Character dimensions
    char_width: f32,
    char_height: f32,
    font_size: f32,
    // Keyboard modifiers
    modifiers: ModifiersState,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let font = SystemSource::new()
            .select_best_match(
                &[
                    FamilyName::Title("CaskaydiaCove Nerd Font Mono".to_string()),
                    FamilyName::Title("CaskaydiaCove NF Mono".to_string()),
                    FamilyName::Monospace,
                ],
                &Properties::new(),
            )
            .ok()
            .and_then(|handle| handle.load().ok());

        let shell = Shell::new(80, 24).ok();
        if shell.is_none() {
            eprintln!("Failed to create shell");
        }

        // Calculate character dimensions (will be updated when window is created)
        let font_size = 16.0;
        let char_width = 9.0; // Approximate for monospace
        let char_height = 20.0;

        Self {
            window: None,
            surface: None,
            terminal: Terminal::new(80, 24),
            font,
            shell,
            char_width,
            char_height,
            font_size,
            modifiers: ModifiersState::empty(),
        }
    }

    /// Calculate grid dimensions based on window size
    fn calculate_grid_size(&self, window_width: u32, window_height: u32) -> (usize, usize) {
        let cols = ((window_width as f32 - 20.0) / self.char_width).floor() as usize;
        let rows = ((window_height as f32 - 40.0) / self.char_height).floor() as usize;
        (cols.max(10), rows.max(3)) // Minimum 10x3
    }

    /// Resize terminal grid and shell
    fn resize_terminal(&mut self, cols: usize, rows: usize) {
        // Resize terminal (preserves existing content and clamps cursor)
        self.terminal.resize(cols, rows);

        // Update shell PTY size
        if let Some(shell) = &mut self.shell
            && let Err(e) = shell.resize(cols as u16, rows as u16)
        {
            eprintln!("Failed to resize shell: {}", e);
        }
    }

    fn process_shell_output(&mut self) -> bool {
        // Check for shell output from the reader thread (non-blocking)
        // Returns false if the child process has exited
        if let Some(ref shell) = self.shell {
            let mut has_data = false;

            // Drain all available messages from the channel
            loop {
                match shell.receiver.try_recv() {
                    Ok(data) => {
                        has_data = true;
                        // Process bytes through the terminal (VTE parser + state updates)
                        self.terminal.process_bytes(&data);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // No more data available right now
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Channel closed - child process has exited
                        eprintln!("Child process exited");
                        return false;
                    }
                }
            }

            if has_data {
                self.terminal.state_mut().grid.viewport_to_end();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
        true
    }

    fn render(&mut self) -> Result<()> {
        let surface = self.surface.as_mut().context("No surface available")?;
        let window = self.window.as_ref().context("No window available")?;
        let size = window.inner_size();

        let width = size.width as i32;
        let height = size.height as i32;

        let w = NonZeroU32::new(size.width).context("Window width is zero")?;
        let h = NonZeroU32::new(size.height).context("Window height is zero")?;

        surface
            .resize(w, h)
            .map_err(|e| anyhow::anyhow!("Failed to resize surface: {:?}", e))?;

        let mut dt = DrawTarget::new(width, height);
        dt.clear(SolidSource::from_unpremultiplied_argb(0xff, 0, 0, 0));

        if let Some(font) = &self.font {
            let offset_x = 10.0;
            let offset_y = 20.0;

            let viewport = self.terminal.state().grid.get_viewport();
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
                        if font.glyph_for_char(cell.ch).is_some() {
                            dt.draw_text(
                                font,
                                self.font_size,
                                &text,
                                raqote::Point::new(x, y),
                                &Source::Solid(SolidSource::from_unpremultiplied_argb(
                                    0xff, cell.fg.r, cell.fg.g, cell.fg.b,
                                )),
                                &raqote::DrawOptions::new(),
                            );
                        }
                    }
                }
            }

            // Draw cursor
            // Calculate cursor position relative to viewport
            let cursor_viewport_row = self
                .terminal
                .state()
                .cursor
                .row
                .saturating_sub(self.terminal.state().grid.viewport_start);
            if cursor_viewport_row < self.terminal.state().grid.viewport_height {
                let cursor_x = offset_x + self.terminal.state().cursor.col as f32 * self.char_width;
                let cursor_y = offset_y + cursor_viewport_row as f32 * self.char_height;

                // Draw cursor as a filled rectangle (block cursor)
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
        }

        let dt_data = dt.get_data();
        let mut buffer = surface
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

    fn handle_keyboard_input(&mut self, key: &Key, text: Option<&str>) {
        if let Some(shell) = &mut self.shell {
            let bytes = match key {
                Key::Named(named) => match named {
                    NamedKey::Enter => Some(b"\r".to_vec()),
                    NamedKey::Backspace => Some(b"\x7f".to_vec()),
                    NamedKey::Tab => Some(b"\t".to_vec()),
                    NamedKey::Space => Some(b" ".to_vec()),
                    NamedKey::Escape => Some(b"\x1b".to_vec()),
                    NamedKey::ArrowUp => Some(b"\x1b[A".to_vec()),
                    NamedKey::ArrowDown => Some(b"\x1b[B".to_vec()),
                    NamedKey::ArrowRight => Some(b"\x1b[C".to_vec()),
                    NamedKey::ArrowLeft => Some(b"\x1b[D".to_vec()),
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
                        if self.modifiers.control_key() && ch.is_ascii_alphabetic() {
                            // Ctrl+letter produces control codes 1-26
                            // Ctrl+A = 1, Ctrl+B = 2, ..., Ctrl+Z = 26
                            // Ctrl+R = 18 (0x12) triggers reverse history search in shells
                            let lower = ch.to_ascii_lowercase();
                            let ctrl_code = (lower as u8) - b'a' + 1;
                            Some(vec![ctrl_code])
                        } else if let Some(text_str) = text {
                            // Normal character - use the text provided by winit
                            Some(text_str.as_bytes().to_vec())
                        } else {
                            // Fallback - send the character as-is
                            Some(s.as_bytes().to_vec())
                        }
                    } else if let Some(text_str) = text {
                        // Multi-character string - use text from winit
                        Some(text_str.as_bytes().to_vec())
                    } else {
                        Some(s.as_bytes().to_vec())
                    }
                }
                _ => None,
            };

            if let Some(data) = bytes
                && let Err(e) = shell.write(&data)
            {
                eprintln!("Failed to write to shell: {}", e);
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            println!("Creating window...");
            let window_attrs = Window::default_attributes()
                .with_title("Rustty Terminal")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

            let window = match event_loop.create_window(window_attrs) {
                Ok(w) => Rc::new(w),
                Err(e) => {
                    eprintln!("Failed to create window: {}", e);
                    event_loop.exit();
                    return;
                }
            };
            println!("Window created");

            let context = match Context::new(window.clone()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create context: {}", e);
                    event_loop.exit();
                    return;
                }
            };

            let surface = match Surface::new(&context, window.clone()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to create surface: {}", e);
                    event_loop.exit();
                    return;
                }
            };
            println!("Surface created");

            // Calculate initial grid size based on window dimensions
            let size = window.inner_size();
            let (cols, rows) = self.calculate_grid_size(size.width, size.height);
            println!("Calculated grid size: {}x{}", cols, rows);

            self.window = Some(window);
            self.surface = Some(surface);

            // Resize terminal to match window
            self.resize_terminal(cols, rows);

            println!("Rendering initial frame...");
            if let Err(e) = self.render() {
                eprintln!("Initial render error: {}", e);
            }
            println!("Initial render complete");
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Check for PTY data from reader thread
        // If the child process has exited, close the terminal
        if !self.process_shell_output() {
            eprintln!("Child process terminated, exiting...");
            event_loop.exit();
            return;
        }

        // Run at ~60fps (16ms intervals)
        //
        // Note: This is NOT "polling the PTY" - that happens in a separate blocking thread.
        // This is only checking a Rust channel with try_recv(), which is essentially free
        // (just an atomic load). The architecture is:
        //
        // 1. PTY reader thread: Blocks on read() - zero CPU when idle
        // 2. Main thread: Checks channel every 16ms - <0.1% CPU
        // 3. When PTY has data, thread wakes, sends to channel, we process it
        //
        // This is the same pattern used by production terminals like Alacritty.
        // Alternative approaches (mio, manual event loop integration) are more complex
        // and don't provide significant benefits since winit can't be woken from threads.
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
                if let Err(e) = self.render() {
                    eprintln!("Render error: {}", e);
                }
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let text = event.text.as_ref().map(|s| s.as_str());
                    self.handle_keyboard_input(&event.logical_key, text);
                }
            }
            WindowEvent::Resized(new_size) => {
                let (cols, rows) = self.calculate_grid_size(new_size.width, new_size.height);
                println!(
                    "Window resized to: {}x{} -> grid: {}x{}",
                    new_size.width, new_size.height, cols, rows
                );
                self.resize_terminal(cols, rows);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
