// Copyright (c) Vitaly Golik (mjojo)

//! Font registry: loading, fallback chain, cache of glyph metrics and rasters.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Font identifier in the registry (matches `GlyphRun::font`).
pub type FontId = u32;

/// Glyph raster: coverage 0..255, `w*h` bytes, row by row from top to bottom.
#[derive(Debug)]
pub struct GlyphBitmap {
    /// Offset of the left edge from the pen along X.
    pub xmin: i32,
    /// Offset of the bottom edge from the baseline upwards (as in fontdue).
    pub ymin: i32,
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
    pub ascent: f32,
    /// Negative value (downwards from the baseline).
    pub descent: f32,
    pub line_gap: f32,
}

impl LineMetrics {
    pub fn height(&self) -> f32 {
        self.ascent - self.descent + self.line_gap
    }
}

#[derive(Debug)]
pub enum FontError {
    Io(std::io::Error),
    Parse(&'static str),
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::Io(e) => write!(f, "failed to read font: {e}"),
            FontError::Parse(e) => write!(f, "invalid font: {e}"),
        }
    }
}
impl std::error::Error for FontError {}

#[derive(Default)]
pub struct Fonts {
    fonts: Vec<fontdue::Font>,
    cache: Mutex<HashMap<(FontId, u16, u32), Arc<GlyphBitmap>>>,
}

impl Fonts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_bytes(&mut self, data: &[u8]) -> Result<FontId, FontError> {
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).map_err(FontError::Parse)?;
        self.fonts.push(font);
        Ok((self.fonts.len() - 1) as FontId)
    }

    pub fn add_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<FontId, FontError> {
        let data = std::fs::read(path).map_err(FontError::Io)?;
        self.add_bytes(&data)
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Glyph index for `ch`; `None` if the font does not contain it.
    pub fn glyph_index(&self, font: FontId, ch: char) -> Option<u16> {
        let i = self.fonts.get(font as usize)?.lookup_glyph_index(ch);
        (i != 0).then_some(i)
    }

    /// First font in the chain that contains `ch`; otherwise the first font (its .notdef).
    pub fn pick(&self, chain: &[FontId], ch: char) -> Option<(FontId, u16)> {
        chain.iter().find_map(|&f| self.glyph_index(f, ch).map(|g| (f, g))).or_else(|| chain.first().map(|&f| (f, 0)))
    }

    pub fn advance(&self, font: FontId, glyph: u16, size_px: f32) -> f32 {
        self.fonts[font as usize].metrics_indexed(glyph, size_px).advance_width
    }

    pub fn line_metrics(&self, font: FontId, size_px: f32) -> LineMetrics {
        match self.fonts[font as usize].horizontal_line_metrics(size_px) {
            Some(m) => LineMetrics { ascent: m.ascent, descent: m.descent, line_gap: m.line_gap },
            None => LineMetrics { ascent: size_px * 0.8, descent: -size_px * 0.2, line_gap: 0.0 },
        }
    }

    /// Glyph raster cached by (font, glyph, size).
    pub fn bitmap(&self, font: FontId, glyph: u16, size_px: f32) -> Arc<GlyphBitmap> {
        let key = (font, glyph, size_px.to_bits());
        let mut cache = self.cache.lock().unwrap();
        cache
            .entry(key)
            .or_insert_with(|| {
                let (m, data) = self.fonts[font as usize].rasterize_indexed(glyph, size_px);
                Arc::new(GlyphBitmap { xmin: m.xmin, ymin: m.ymin, w: m.width as u32, h: m.height as u32, data })
            })
            .clone()
    }
}
