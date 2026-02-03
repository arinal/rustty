// Vertex shader

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.fg_color = model.fg_color;
    out.bg_color = model.bg_color;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
    return out;
}

// Fragment shader

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(t_diffuse, s_diffuse, in.tex_coords).r;

    // If bg_color has alpha=0, this is a background-only quad - render solid bg
    if (in.bg_color.a < 0.5) {
        return vec4<f32>(in.bg_color.rgb, 1.0);
    }

    // Otherwise, render glyph with actual alpha for proper framebuffer blending
    return vec4<f32>(in.fg_color.rgb, alpha);
}
