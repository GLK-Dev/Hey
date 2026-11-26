// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Color, Point, Rect};

/// Border; centered on the rectangle edge (ADR rule).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

/// Shadow: offset, blur radius, color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset: Point,
    pub blur: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

/// A single glyph: font-local identifier and baseline position (physical pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: u32,
    pub size_px: f32,
    pub color: Color,
    pub glyphs: Vec<Glyph>,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DrawCmd {
    /// `chamfer` is a 45° cut (px) on the top-left and bottom-right corners only (design from design/).
    Rect {
        rect: Rect,
        fill: Color,
        border: Option<Border>,
        chamfer: f32,
        shadow: Option<Shadow>,
    },
    Line {
        a: Point,
        b: Point,
        width: f32,
        color: Color,
    },
    Text(GlyphRun),
    Image {
        rect: Rect,
        image: ImageId,
    },
    ClipPush(Rect),
    ClipPop,
}

impl DrawCmd {
    /// A plain filled rectangle without border, chamfer or shadow.
    pub fn fill(rect: Rect, color: Color) -> Self {
        DrawCmd::Rect { rect, fill: color, border: None, chamfer: 0.0, shadow: None }
    }
}
