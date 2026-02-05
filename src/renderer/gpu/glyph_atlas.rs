use anyhow::Result;

#[derive(Clone, Copy)]
pub(super) struct AtlasPosition {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Glyph Atlas for efficient text rendering
pub(super) struct GlyphAtlas {
    pub texture: wgpu::Texture,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
    next_x: u32,
    next_y: u32,
    row_height: u32,
    cache: std::collections::HashMap<char, AtlasPosition>,
    cell_width: u32,
    cell_height: u32,
    baseline_y: f32,
}

impl GlyphAtlas {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        font: &font_kit::font::Font,
        cell_width: u32,
        cell_height: u32,
    ) -> Result<Self> {
        let width = 2048;
        let height = 2048;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create texture view
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("Glyph Atlas Bind Group Layout"),
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Glyph Atlas Bind Group"),
        });

        // Calculate baseline
        let metrics = font.metrics();
        let units_per_em = metrics.units_per_em as f32;
        let ascent = metrics.ascent / units_per_em;
        let font_size = 16.0;
        let baseline_y = (ascent * font_size).ceil();

        Ok(Self {
            texture,
            bind_group_layout,
            bind_group,
            width,
            height,
            next_x: 0,
            next_y: 0,
            row_height: 0,
            cache: std::collections::HashMap::new(),
            cell_width,
            cell_height,
            baseline_y,
        })
    }

    pub fn get_or_rasterize(
        &mut self,
        ch: char,
        font: &font_kit::font::Font,
        queue: &wgpu::Queue,
    ) -> Result<AtlasPosition> {
        if let Some(pos) = self.cache.get(&ch) {
            return Ok(*pos);
        }

        // Rasterize glyph
        let glyph_id = font
            .glyph_for_char(ch)
            .unwrap_or(font.glyph_for_char(' ').unwrap());

        use font_kit::canvas::{Canvas, Format, RasterizationOptions};
        use font_kit::hinting::HintingOptions;
        use pathfinder_geometry::transform2d::Transform2F;
        use pathfinder_geometry::vector::{Vector2F, Vector2I};

        // Use fixed cell size for all glyphs - ensures consistent UV mapping
        let canvas_size = Vector2I::new(self.cell_width as i32, self.cell_height as i32);
        let mut canvas = Canvas::new(canvas_size, Format::A8);

        // Position all glyphs at baseline
        let transform = Transform2F::from_translation(Vector2F::new(0.0, self.baseline_y));

        let font_size = 16.0;
        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            font_size,
            transform,
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        )?;

        // Find position in atlas
        if self.next_x + self.cell_width > self.width {
            self.next_x = 0;
            self.next_y += self.row_height;
            self.row_height = 0;
        }

        if self.next_y + self.cell_height > self.height {
            anyhow::bail!("Glyph atlas full");
        }

        let pos = AtlasPosition {
            x: self.next_x,
            y: self.next_y,
            width: self.cell_width,
            height: self.cell_height,
        };

        // Upload to GPU
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

        self.next_x += self.cell_width;
        self.row_height = self.row_height.max(self.cell_height);

        self.cache.insert(ch, pos);
        Ok(pos)
    }
}
