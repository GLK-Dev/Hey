// Copyright (c) Vitaly Golik (mjojo)

//! Реестр шрифтов: загрузка, fallback-цепочка, кэш метрик и растров глифов.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Идентификатор шрифта в реестре (совпадает с `GlyphRun::font`).
pub type FontId = u32;

/// Растр глифа: покрытие 0..255, `w*h` байт, построчно сверху вниз.
#[derive(Debug)]
pub struct GlyphBitmap {
    /// Смещение левого края от пера по X.
    pub xmin: i32,
    /// Смещение нижнего края от базовой линии вверх (как в fontdue).
    pub ymin: i32,
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
    pub ascent: f32,
    /// Отрицательное значение (вниз от базовой линии).
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
            FontError::Io(e) => write!(f, "ошибка чтения шрифта: {e}"),
            FontError::Parse(e) => write!(f, "некорректный шрифт: {e}"),
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

    /// Индекс глифа для `ch`; `None`, если шрифт его не содержит.
    pub fn glyph_index(&self, font: FontId, ch: char) -> Option<u16> {
        let i = self.fonts.get(font as usize)?.lookup_glyph_index(ch);
        (i != 0).then_some(i)
    }

    /// Первый шрифт цепочки, содержащий `ch`; иначе первый шрифт (его .notdef).
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

    /// Растр глифа с кэшированием по (шрифт, глиф, размер).
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
