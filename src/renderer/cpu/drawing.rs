//! Drawing primitives for CPU rendering
//!
//! Helper functions for drawing backgrounds, cursor shapes, and text decorations.

use raqote::{DrawOptions, DrawTarget, Path, PathOp, Point, SolidSource, Source, StrokeStyle};

/// Draw a solid background rectangle
pub(super) fn draw_background(
    dt: &mut DrawTarget,
    x: f32,
    y: f32,
    width: f32,
    r: u8,
    g: u8,
    b: u8,
) {
    let bg_rect = Path {
        ops: vec![
            PathOp::MoveTo(Point::new(x, y - 15.0)),
            PathOp::LineTo(Point::new(x + width, y - 15.0)),
            PathOp::LineTo(Point::new(x + width, y + 5.0)),
            PathOp::LineTo(Point::new(x, y + 5.0)),
            PathOp::Close,
        ],
        winding: raqote::Winding::NonZero,
    };
    dt.fill(
        &bg_rect,
        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, r, g, b)),
        &DrawOptions::new(),
    );
}

/// Draw underline beneath text
pub(super) fn draw_underline(dt: &mut DrawTarget, x: f32, y: f32, width: f32, r: u8, g: u8, b: u8) {
    let underline_y = y + 2.0;
    let underline_path = Path {
        ops: vec![
            PathOp::MoveTo(Point::new(x, underline_y)),
            PathOp::LineTo(Point::new(x + width, underline_y)),
        ],
        winding: raqote::Winding::NonZero,
    };
    dt.stroke(
        &underline_path,
        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, r, g, b)),
        &StrokeStyle {
            width: 1.0,
            ..Default::default()
        },
        &DrawOptions::new(),
    );
}

/// Draw block cursor
pub(super) fn draw_block_cursor(dt: &mut DrawTarget, x: f32, y: f32, width: f32) {
    let cursor_rect = Path {
        ops: vec![
            PathOp::MoveTo(Point::new(x, y - 15.0)),
            PathOp::LineTo(Point::new(x + width, y - 15.0)),
            PathOp::LineTo(Point::new(x + width, y + 5.0)),
            PathOp::LineTo(Point::new(x, y + 5.0)),
            PathOp::Close,
        ],
        winding: raqote::Winding::NonZero,
    };
    dt.fill(
        &cursor_rect,
        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, 255, 255, 255)),
        &DrawOptions::new(),
    );
}

/// Draw underline cursor
pub(super) fn draw_underline_cursor(dt: &mut DrawTarget, x: f32, y: f32, width: f32) {
    let underline_y = y + 3.0;
    let underline_path = Path {
        ops: vec![
            PathOp::MoveTo(Point::new(x, underline_y)),
            PathOp::LineTo(Point::new(x + width, underline_y)),
        ],
        winding: raqote::Winding::NonZero,
    };
    dt.stroke(
        &underline_path,
        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, 255, 255, 255)),
        &StrokeStyle {
            width: 2.0,
            ..Default::default()
        },
        &DrawOptions::new(),
    );
}

/// Draw bar cursor
pub(super) fn draw_bar_cursor(dt: &mut DrawTarget, x: f32, y: f32) {
    let bar_path = Path {
        ops: vec![
            PathOp::MoveTo(Point::new(x, y - 15.0)),
            PathOp::LineTo(Point::new(x, y + 5.0)),
        ],
        winding: raqote::Winding::NonZero,
    };
    dt.stroke(
        &bar_path,
        &Source::Solid(SolidSource::from_unpremultiplied_argb(0xff, 255, 255, 255)),
        &StrokeStyle {
            width: 2.0,
            ..Default::default()
        },
        &DrawOptions::new(),
    );
}

/// Apply bold effect by brightening colors
pub(super) fn apply_bold(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let brighten = |c: u8| -> u8 { ((c as u16 * 3 / 2).min(255)) as u8 };
    (brighten(r), brighten(g), brighten(b))
}

/// Apply italic effect by adding cyan tint
pub(super) fn apply_italic(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (
        r,
        ((g as u16 + 30).min(255)) as u8,
        ((b as u16 + 30).min(255)) as u8,
    )
}
