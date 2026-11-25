// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Color, Point, Rect};

/// Рамка; центрируется на границе прямоугольника (правило ADR).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

/// Тень: смещение, радиус размытия, цвет.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset: Point,
    pub blur: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

/// Один глиф: идентификатор в шрифте и позиция базовой линии (физические пиксели).
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
pub enum DrawCmd {
    /// `chamfer` — срез 45° (px) только у верхнего-левого и нижнего-правого углов (дизайн из design/).
    Rect {
        rect: Rect,
        fill: Color,
        border: Option<Border>,
        chamfer: f32,
        shadow: Option<Shadow>,
    },
    Line { a: Point, b: Point, width: f32, color: Color },
    Text(GlyphRun),
    Image { rect: Rect, image: ImageId },
    ClipPush(Rect),
    ClipPop,
}

impl DrawCmd {
    /// Простой залитый прямоугольник без рамки, среза и тени.
    pub fn fill(rect: Rect, color: Color) -> Self {
        DrawCmd::Rect { rect, fill: color, border: None, chamfer: 0.0, shadow: None }
    }
}
