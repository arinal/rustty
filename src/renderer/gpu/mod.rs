//! GPU-accelerated renderer using wgpu
//!
//! This module provides a hardware-accelerated rendering backend for better performance
//! on large terminals and smooth scrolling.

mod glyph_atlas;
mod vertex;

use anyhow::{Context as _, Result};
use std::sync::Arc;
use winit::window::Window;

use glyph_atlas::{AtlasPosition, GlyphAtlas};
use vertex::Vertex;

pub struct GpuRenderer {
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
    offset_x: f32,
    offset_y: f32,
}

impl GpuRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
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
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Load font (needed for glyph atlas)
        let font = font_kit::source::SystemSource::new()
            .select_best_match(
                &[
                    font_kit::family_name::FamilyName::Title(
                        "CaskaydiaCove Nerd Font Mono".to_string(),
                    ),
                    font_kit::family_name::FamilyName::Title("CaskaydiaCove NF Mono".to_string()),
                    font_kit::family_name::FamilyName::Monospace,
                ],
                &font_kit::properties::Properties::new(),
            )
            .context("Failed to find suitable font")?
            .load()
            .context("Failed to load font")?;

        let char_width = 9.0;
        let char_height = 20.0;

        // Create glyph atlas (must be before pipeline creation)
        let glyph_atlas = GlyphAtlas::new(
            &device,
            &queue,
            &font,
            char_width as u32,
            char_height as u32,
        )?;

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terminal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terminal.wgsl").into()),
        });

        // Create render pipeline with glyph atlas bind group layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terminal Pipeline Layout"),
            bind_group_layouts: &[&glyph_atlas.bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terminal Pipeline"),
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

        // Create vertex buffer (will be resized dynamically)
        let initial_capacity = 80 * 24 * 6; // 80x24 grid, 6 vertices per character
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (initial_capacity * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let offset_x = 10.0;
        let offset_y = 20.0;

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
            offset_x,
            offset_y,
        })
    }

    pub fn char_dimensions(&self) -> (f32, f32) {
        (self.char_width, self.char_height)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
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

    /// Render with custom cursor visibility
    ///
    /// This method allows the caller to control cursor visibility (e.g., for blinking).
    pub fn render_with_blink(
        &mut self,
        state: &crate::TerminalState,
        cursor_visible: bool,
    ) -> Result<()> {
        let viewport = state.grid.get_viewport();
        let cursor = &state.cursor;
        // Get current surface texture
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build vertex buffer for all visible characters
        let mut vertices = Vec::new();

        // Render text cells
        for (row_idx, row) in viewport.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let x = self.offset_x + col_idx as f32 * self.char_width;
                let y = self.offset_y + row_idx as f32 * self.char_height;

                // Convert to NDC coordinates
                let x_ndc = (x / self.config.width as f32) * 2.0 - 1.0;
                let y_ndc = 1.0 - (y / self.config.height as f32) * 2.0;
                let w_ndc = (self.char_width / self.config.width as f32) * 2.0;
                let h_ndc = (self.char_height / self.config.height as f32) * 2.0;

                // Calculate colors
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
                    1.0,
                ];

                // Check for block drawing characters (render procedurally)
                if let Some((top_color, bottom_color)) =
                    self.get_block_char_colors(cell.ch, fg_color, bg_color)
                {
                    // Get atlas position for solid block (use space ' ' as solid)
                    let solid_atlas_pos =
                        self.glyph_atlas
                            .get_or_rasterize(' ', &self.font, &self.queue)?;

                    // Render top half
                    self.add_quad_vertices(
                        &mut vertices,
                        x_ndc,
                        y_ndc,
                        w_ndc,
                        h_ndc / 2.0,
                        &solid_atlas_pos,
                        [0.0, 0.0, 0.0, 0.0], // Transparent fg
                        top_color,            // Actual color in bg
                    );

                    // Render bottom half
                    self.add_quad_vertices(
                        &mut vertices,
                        x_ndc,
                        y_ndc - h_ndc / 2.0,
                        w_ndc,
                        h_ndc / 2.0,
                        &solid_atlas_pos,
                        [0.0, 0.0, 0.0, 0.0], // Transparent fg
                        bottom_color,         // Actual color in bg
                    );
                } else {
                    // Normal character rendering
                    if cell.ch != ' ' && !cell.ch.is_control() {
                        // Get or rasterize glyph
                        let atlas_pos =
                            self.glyph_atlas
                                .get_or_rasterize(cell.ch, &self.font, &self.queue)?;

                        // Apply text attributes
                        let mut fg = fg_color;
                        if cell.bold {
                            // Brighten colors for bold
                            fg[0] = (fg[0] * 1.5).min(1.0);
                            fg[1] = (fg[1] * 1.5).min(1.0);
                            fg[2] = (fg[2] * 1.5).min(1.0);
                        }
                        if cell.italic {
                            // Add cyan tint for italic
                            fg[1] = (fg[1] + 0.12).min(1.0);
                            fg[2] = (fg[2] + 0.12).min(1.0);
                        }

                        self.add_quad_vertices(
                            &mut vertices,
                            x_ndc,
                            y_ndc,
                            w_ndc,
                            h_ndc,
                            &atlas_pos,
                            fg,
                            bg_color,
                        );
                    } else {
                        // Background only
                        let atlas_pos =
                            self.glyph_atlas
                                .get_or_rasterize(' ', &self.font, &self.queue)?;
                        self.add_quad_vertices(
                            &mut vertices,
                            x_ndc,
                            y_ndc,
                            w_ndc,
                            h_ndc,
                            &atlas_pos,
                            [0.0, 0.0, 0.0, 0.0],
                            bg_color,
                        );
                    }
                }
            }
        }

        // Render cursor
        if cursor_visible {
            let x = self.offset_x + cursor.col as f32 * self.char_width;
            let y = self.offset_y + cursor.row as f32 * self.char_height;

            let x_ndc = (x / self.config.width as f32) * 2.0 - 1.0;
            let y_ndc = 1.0 - (y / self.config.height as f32) * 2.0;
            let w_ndc = (self.char_width / self.config.width as f32) * 2.0;
            let h_ndc = (self.char_height / self.config.height as f32) * 2.0;

            let cursor_color = [1.0, 1.0, 1.0, 1.0];
            let solid_atlas_pos =
                self.glyph_atlas
                    .get_or_rasterize(' ', &self.font, &self.queue)?;

            use crate::CursorStyle;

            match cursor.style {
                CursorStyle::Block => {
                    self.add_quad_vertices(
                        &mut vertices,
                        x_ndc,
                        y_ndc,
                        w_ndc,
                        h_ndc,
                        &solid_atlas_pos,
                        [0.0, 0.0, 0.0, 0.0],
                        cursor_color,
                    );
                }
                CursorStyle::Underline => {
                    let underline_height = h_ndc * 0.15;
                    self.add_quad_vertices(
                        &mut vertices,
                        x_ndc,
                        y_ndc - h_ndc + underline_height,
                        w_ndc,
                        underline_height,
                        &solid_atlas_pos,
                        [0.0, 0.0, 0.0, 0.0],
                        cursor_color,
                    );
                }
                CursorStyle::Bar => {
                    let bar_width = w_ndc * 0.15;
                    self.add_quad_vertices(
                        &mut vertices,
                        x_ndc,
                        y_ndc,
                        bar_width,
                        self.char_height,
                        &solid_atlas_pos,
                        [0.0, 0.0, 0.0, 0.0],
                        [1.0, 1.0, 1.0, 0.0], // a=0 for solid rendering
                    );
                }
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
                self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Vertex Buffer"),
                    size: vertex_data.len() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }

            self.queue.write_buffer(&self.vertex_buffer, 0, vertex_data);
        }

        // Render
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
            if !vertices.is_empty() {
                render_pass.draw(0..vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn add_quad_vertices(
        &self,
        vertices: &mut Vec<Vertex>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        atlas_pos: &AtlasPosition,
        fg_color: [f32; 4],
        bg_color: [f32; 4],
    ) {
        let u0 = atlas_pos.x as f32 / self.glyph_atlas.width as f32;
        let v0 = atlas_pos.y as f32 / self.glyph_atlas.height as f32;
        let u1 = (atlas_pos.x + atlas_pos.width) as f32 / self.glyph_atlas.width as f32;
        let v1 = (atlas_pos.y + atlas_pos.height) as f32 / self.glyph_atlas.height as f32;

        // Two triangles forming a quad
        vertices.extend_from_slice(&[
            Vertex {
                position: [x, y],
                tex_coords: [u0, v0],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x + w, y],
                tex_coords: [u1, v0],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x, y - h],
                tex_coords: [u0, v1],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x + w, y],
                tex_coords: [u1, v0],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x + w, y - h],
                tex_coords: [u1, v1],
                fg_color,
                bg_color,
            },
            Vertex {
                position: [x, y - h],
                tex_coords: [u0, v1],
                fg_color,
                bg_color,
            },
        ]);
    }
}

// Implement rustty::renderer::Renderer trait for GpuRenderer
impl super::Renderer for GpuRenderer {
    fn char_dimensions(&self) -> (f32, f32) {
        // Use existing method
        GpuRenderer::char_dimensions(self)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        // Use existing method
        GpuRenderer::resize(self, width, height)
    }

    fn render(&mut self, state: &crate::TerminalState) -> anyhow::Result<()> {
        // Default to visible cursor for trait method
        self.render_with_blink(state, true)
    }

    fn render_with_blink(
        &mut self,
        state: &crate::TerminalState,
        cursor_visible: bool,
    ) -> anyhow::Result<()> {
        // Delegate to the public method
        GpuRenderer::render_with_blink(self, state, cursor_visible)
    }

    fn is_initialized(&self) -> bool {
        // GPU renderer is always initialized once created
        true
    }
}
