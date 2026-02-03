use anyhow::{Context as _, Result};
use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use rustty::TerminalSession;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    session: TerminalSession,
    modifiers: ModifiersState,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let session = TerminalSession::new(80, 24).expect("Failed to create terminal session");

        Self {
            window: None,
            renderer: None,
            session,
            modifiers: ModifiersState::empty(),
        }
    }

    fn calculate_grid_size(&self, window_width: u32, window_height: u32) -> (usize, usize) {
        if let Some(renderer) = &self.renderer {
            let (char_width, char_height) = renderer.char_dimensions();
            let cols = ((window_width as f32 - 20.0) / char_width).floor() as usize;
            let rows = ((window_height as f32 - 40.0) / char_height).floor() as usize;
            (cols.max(10), rows.max(3))
        } else {
            (80, 24) // Default fallback
        }
    }

    fn process_shell_output(&mut self) -> bool {
        let still_running = self.session.process_output();

        if let Some(window) = &self.window {
            window.request_redraw();
        }

        still_running
    }

    fn render(&mut self) -> Result<()> {
        let renderer = self.renderer.as_mut().context("No renderer available")?;

        // Get terminal state for rendering
        let state = self.session.state();
        let viewport = state.grid.get_viewport();

        // Calculate cursor position relative to viewport
        let cursor_viewport_row = state.cursor.row.saturating_sub(state.grid.viewport_start);
        let cursor_visible = cursor_viewport_row < state.grid.viewport_height;

        renderer.render_frame(viewport, &state.cursor, cursor_visible)?;
        Ok(())
    }

    fn handle_keyboard_input(&mut self, key: &Key, text: Option<&str>) {
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

                    if self.modifiers.control_key() && ch.is_ascii_alphabetic() {
                        let lower = ch.to_ascii_lowercase();
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
        };

        if let Some(data) = bytes
            && let Err(e) = self.session.write_input(&data)
        {
            eprintln!("Failed to write to shell: {}", e);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            println!("Creating window...");
            let window_attrs = Window::default_attributes()
                .with_title("Rustty Terminal (GPU)")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

            let window = match event_loop.create_window(window_attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    eprintln!("Failed to create window: {}", e);
                    event_loop.exit();
                    return;
                }
            };
            println!("Window created");

            // Initialize GPU renderer
            match pollster::block_on(GpuRenderer::new(window.clone())) {
                Ok(renderer) => {
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

                    self.window = Some(window);
                    self.renderer = Some(renderer);
                    self.session.resize(cols, rows);

                    println!("Rendering initial frame...");
                    if let Err(e) = self.render() {
                        eprintln!("Initial render error: {}", e);
                    }
                    println!("Initial render complete");
                }
                Err(e) => {
                    eprintln!("Failed to initialize GPU renderer: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.process_shell_output() {
            eprintln!("Child process terminated, exiting...");
            event_loop.exit();
            return;
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
                self.session.resize(cols, rows);

                if let Some(renderer) = &mut self.renderer
                    && let Err(e) = renderer.resize(new_size.width, new_size.height)
                {
                    eprintln!("Failed to resize renderer: {}", e);
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

// GPU Renderer implementation
struct GpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    glyph_atlas: GlyphAtlas,
    font: font_kit::font::Font,
    char_width: f32,
    char_height: f32,
    font_size: f32,
    offset_x: f32,
    offset_y: f32,
}

impl GpuRenderer {
    async fn new(window: Arc<Window>) -> Result<Self> {
        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone())?;

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find an appropriate adapter")?;

        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        // Configure surface
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .context("Surface isn't supported by the adapter")?;

        // Use non-sRGB format since terminal colors are already in sRGB space
        // Using sRGB format would apply gamma correction twice, making colors pale
        config.format = wgpu::TextureFormat::Bgra8Unorm;
        surface.configure(&device, &config);

        // Load font
        let font_handle = SystemSource::new()
            .select_best_match(
                &[
                    FamilyName::Title("CaskaydiaCove Nerd Font Mono".to_string()),
                    FamilyName::Title("CaskaydiaCove NF Mono".to_string()),
                    FamilyName::Monospace,
                ],
                &Properties::new(),
            )
            .context("Failed to find font")?;

        let font = font_handle.load().context("Failed to load font")?;
        eprintln!("Loaded font: {:?}", font.family_name());

        // Calculate character dimensions from font metrics
        let font_size = 16.0;
        let metrics = font.metrics();

        eprintln!(
            "Font metrics - units_per_em: {}, ascent: {}, descent: {}, line_gap: {}",
            metrics.units_per_em, metrics.ascent, metrics.descent, metrics.line_gap
        );

        // Scale factor to convert font units to pixels
        let scale = font_size / metrics.units_per_em as f32;

        // Use advance width for monospace font cell width
        let m_glyph_id = font.glyph_for_char('M').context("No glyph for 'M'")?;
        let char_width = font
            .advance(m_glyph_id)
            .map(|a| a.x() * scale)
            .unwrap_or(font_size * 0.6)
            .ceil();
        let char_height =
            ((metrics.ascent - metrics.descent + metrics.line_gap).abs() * scale).ceil();
        eprintln!(
            "Character cell dimensions: {}x{} pixels",
            char_width, char_height
        );

        let offset_x = 10.0;
        let offset_y = 20.0;

        // Create glyph atlas with cell dimensions
        let glyph_atlas =
            GlyphAtlas::new(&device, &queue, &font, font_size, char_width, char_height)?;

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terminal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terminal.wgsl").into()),
        });

        // Create render pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&glyph_atlas.bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create vertex buffer (will be recreated if needed when size changes)
        // Initial size for 80x24 terminal: ~2MB is safe
        let initial_buffer_size = 2 * 1024 * 1024; // 2MB
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: initial_buffer_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buffer,
            glyph_atlas,
            font,
            char_width,
            char_height,
            font_size,
            offset_x,
            offset_y,
        })
    }

    fn char_dimensions(&self) -> (f32, f32) {
        (self.char_width, self.char_height)
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
        Ok(())
    }

    /// Returns (top_color, bottom_color) for block drawing characters.
    /// Returns None if not a block character.
    fn get_block_char_colors(
        &self,
        ch: char,
        fg: [f32; 4],
        bg: [f32; 4],
    ) -> Option<([f32; 4], [f32; 4])> {
        match ch {
            // Full block - both halves foreground
            '█' => Some((fg, fg)),
            // Upper half block
            '▀' => Some((fg, bg)),
            // Lower half block
            '▄' => Some((bg, fg)),
            // Light/medium/dark shades - approximate with fg
            '░' | '▒' | '▓' => Some((fg, fg)),
            // Upper 1/8 to 7/8 blocks - approximate
            '▔' => Some((fg, bg)), // Upper 1/8
            // Lower 1/8 to 7/8 blocks - approximate
            '▁' | '▂' | '▃' => Some((bg, fg)), // Lower 1/8 to 3/8
            '▅' | '▆' | '▇' => Some((bg, fg)), // Lower 5/8 to 7/8
            // Left/right blocks - render as full for now
            '▌' => Some((fg, fg)), // Left half
            '▐' => Some((fg, fg)), // Right half
            _ => None,
        }
    }

    fn render_frame(
        &mut self,
        viewport: &[Vec<rustty::Cell>],
        cursor: &rustty::Cursor,
        cursor_visible: bool,
    ) -> Result<()> {
        // Get current surface texture
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build vertex data
        let mut vertices = Vec::new();

        let solid_atlas_pos = AtlasPosition {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };

        // FIRST PASS: Render all backgrounds
        for (row, line) in viewport.iter().enumerate() {
            for (col, cell) in line.iter().enumerate() {
                let x = self.offset_x + col as f32 * self.char_width;
                let y = self.offset_y + row as f32 * self.char_height;

                let bg_color = [
                    cell.bg.r as f32 / 255.0,
                    cell.bg.g as f32 / 255.0,
                    cell.bg.b as f32 / 255.0,
                    0.0, // Signal: this is a background quad
                ];

                // Always render background (even for black - it might be over colored cells)
                self.add_quad(
                    &mut vertices,
                    x,
                    y,
                    self.char_width,
                    self.char_height,
                    &solid_atlas_pos,
                    [0.0, 0.0, 0.0, 0.0], // fg_color unused for backgrounds
                    bg_color,
                );
            }
        }

        // SECOND PASS: Render all glyphs with alpha blending
        for (row, line) in viewport.iter().enumerate() {
            for (col, cell) in line.iter().enumerate() {
                let x = self.offset_x + col as f32 * self.char_width;
                let y = self.offset_y + row as f32 * self.char_height;

                let fg_color = [
                    cell.fg.r as f32 / 255.0,
                    cell.fg.g as f32 / 255.0,
                    cell.fg.b as f32 / 255.0,
                    1.0,
                ];
                let bg_color = [
                    cell.bg.r as f32 / 255.0,
                    cell.bg.g as f32 / 255.0,
                    cell.bg.b as f32 / 255.0,
                    1.0, // Signal: this is a glyph quad
                ];

                // Handle block drawing characters procedurally (U+2580-U+259F)
                if let Some((top_color, bottom_color)) =
                    self.get_block_char_colors(cell.ch, fg_color, bg_color)
                {
                    // Draw as two half-cell rectangles for proper block rendering
                    let half_height = self.char_height / 2.0;
                    // Top half - use bg.a=0 to render as solid
                    let top_bg = [top_color[0], top_color[1], top_color[2], 0.0];
                    self.add_quad(
                        &mut vertices,
                        x,
                        y,
                        self.char_width,
                        half_height,
                        &solid_atlas_pos,
                        top_color,
                        top_bg,
                    );
                    // Bottom half
                    let bottom_bg = [bottom_color[0], bottom_color[1], bottom_color[2], 0.0];
                    self.add_quad(
                        &mut vertices,
                        x,
                        y + half_height,
                        self.char_width,
                        half_height,
                        &solid_atlas_pos,
                        bottom_color,
                        bottom_bg,
                    );
                } else if cell.ch != ' '
                    && !cell.ch.is_control()
                    && let Ok(atlas_pos) = self.glyph_atlas.get_or_insert(
                        cell.ch,
                        &self.device,
                        &self.queue,
                        &self.font,
                        self.font_size,
                    )
                {
                    // Create quad for this character with glyph texture
                    // bg_color.a=1.0 signals glyph rendering with alpha
                    self.add_quad(
                        &mut vertices,
                        x,
                        y,
                        self.char_width,
                        self.char_height,
                        &atlas_pos,
                        fg_color,
                        bg_color,
                    );
                }
            }
        }

        // Render cursor
        if cursor_visible {
            let cursor_viewport_row = cursor.row.saturating_sub(0); // Adjust if needed
            if cursor_viewport_row < viewport.len() {
                let cursor_x = self.offset_x + cursor.col as f32 * self.char_width;
                let cursor_y = self.offset_y + cursor_viewport_row as f32 * self.char_height;

                // Cursor rendered as solid white (bg.a=0 signals solid rendering)
                self.add_quad(
                    &mut vertices,
                    cursor_x,
                    cursor_y,
                    self.char_width,
                    self.char_height,
                    &solid_atlas_pos,
                    [0.0, 0.0, 0.0, 0.0],
                    [1.0, 1.0, 1.0, 0.0], // a=0 for solid rendering
                );
            }
        }

        // Upload vertex data
        if !vertices.is_empty() {
            let vertex_data: &[u8] = bytemuck::cast_slice(&vertices);

            // Check if we need to resize the buffer
            if vertex_data.len() > self.vertex_buffer.size() as usize {
                eprintln!(
                    "Vertex buffer too small ({} bytes), recreating with {} bytes",
                    self.vertex_buffer.size(),
                    vertex_data.len()
                );

                // Recreate buffer with new size (add 50% headroom for future growth)
                let new_size = (vertex_data.len() as f32 * 1.5) as u64;
                self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Vertex Buffer"),
                    size: new_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }

            self.queue.write_buffer(&self.vertex_buffer, 0, vertex_data);
        } else {
            eprintln!("Warning: No vertices to render!");
        }

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.glyph_atlas.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn add_quad(
        &self,
        vertices: &mut Vec<Vertex>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        atlas_pos: &AtlasPosition,
        fg_color: [f32; 4],
        bg_color: [f32; 4],
    ) {
        // Convert screen coords to NDC
        let screen_width = self.config.width as f32;
        let screen_height = self.config.height as f32;

        let x0 = (x / screen_width) * 2.0 - 1.0;
        let y0 = 1.0 - (y / screen_height) * 2.0;
        let x1 = ((x + width) / screen_width) * 2.0 - 1.0;
        let y1 = 1.0 - ((y + height) / screen_height) * 2.0;

        // Texture coordinates in atlas
        let atlas_width = 2048.0; // Match atlas size
        let u0 = atlas_pos.x as f32 / atlas_width;
        let v0 = atlas_pos.y as f32 / atlas_width;
        let u1 = (atlas_pos.x + atlas_pos.width) as f32 / atlas_width;
        let v1 = (atlas_pos.y + atlas_pos.height) as f32 / atlas_width;

        // Two triangles for the quad
        vertices.extend_from_slice(&[
            // Triangle 1
            Vertex {
                position: [x0, y0],
                tex_coords: [u0, v0],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x1, y0],
                tex_coords: [u1, v0],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x0, y1],
                tex_coords: [u0, v1],
                fg_color,
                bg_color,
            },
            // Triangle 2
            Vertex {
                position: [x1, y0],
                tex_coords: [u1, v0],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x1, y1],
                tex_coords: [u1, v1],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x0, y1],
                tex_coords: [u0, v1],
                fg_color,
                bg_color,
            },
        ]);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    fg_color: [f32; 4],
    bg_color: [f32; 4],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

// Glyph Atlas for efficient text rendering
struct GlyphAtlas {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    size: u32,
    glyph_map: HashMap<char, AtlasPosition>,
    next_x: u32,
    next_y: u32,
    cell_width: u32,
    cell_height: u32,
    baseline_y: f32,
}

#[derive(Clone, Copy)]
struct AtlasPosition {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl GlyphAtlas {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font: &font_kit::font::Font,
        font_size: f32,
        cell_width: f32,
        cell_height: f32,
    ) -> Result<Self> {
        let size = 2048;
        let cell_width = cell_width.ceil() as u32;
        let cell_height = cell_height.ceil() as u32;

        // Calculate baseline position within cell
        let metrics = font.metrics();
        let scale = font_size / metrics.units_per_em as f32;
        let baseline_y = metrics.ascent * scale;

        // Create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Clear texture to black
        let clear_data = vec![0u8; (size * size) as usize];
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &clear_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Glyph Atlas Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Glyph Atlas Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(Self {
            texture,
            bind_group,
            bind_group_layout,
            size,
            glyph_map: HashMap::new(),
            next_x: 0,
            next_y: 0,
            cell_width,
            cell_height,
            baseline_y,
        })
    }

    fn get_or_insert(
        &mut self,
        ch: char,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        font: &font_kit::font::Font,
        font_size: f32,
    ) -> Result<AtlasPosition> {
        if let Some(&pos) = self.glyph_map.get(&ch) {
            return Ok(pos);
        }

        // Rasterize glyph using font-kit
        let glyph_id = font.glyph_for_char(ch).context("Character not in font")?;

        use font_kit::canvas::{Canvas, Format, RasterizationOptions};
        use font_kit::hinting::HintingOptions;
        use pathfinder_geometry::transform2d::Transform2F;
        use pathfinder_geometry::vector::{Vector2F, Vector2I};

        // Use fixed cell size for all glyphs - ensures consistent UV mapping
        let canvas_size = Vector2I::new(self.cell_width as i32, self.cell_height as i32);
        let mut canvas = Canvas::new(canvas_size, Format::A8);

        // Position all glyphs at baseline - let the font define the rendering
        let transform = Transform2F::from_translation(Vector2F::new(0.0, self.baseline_y));

        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            font_size,
            transform,
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        )?;

        // Check if we need to move to next row
        if self.next_x + self.cell_width > self.size {
            self.next_x = 0;
            self.next_y += self.cell_height;
        }

        // Check if we're out of space
        if self.next_y + self.cell_height > self.size {
            return Err(anyhow::anyhow!("Glyph atlas is full"));
        }

        let pos = AtlasPosition {
            x: self.next_x,
            y: self.next_y,
            width: self.cell_width,
            height: self.cell_height,
        };

        // Upload rasterized glyph to texture atlas
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: pos.x,
                    y: pos.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &canvas.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.cell_width),
                rows_per_image: Some(self.cell_height),
            },
            wgpu::Extent3d {
                width: self.cell_width,
                height: self.cell_height,
                depth_or_array_layers: 1,
            },
        );

        // Update atlas position tracking
        self.next_x += self.cell_width;

        self.glyph_map.insert(ch, pos);
        Ok(pos)
    }
}
