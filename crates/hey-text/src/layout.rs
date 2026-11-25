// Copyright (c) Vitaly Golik (mjojo)

//! Раскладка текста: измерение, перенос по словам/графемам, выравнивание, многострочность, «…».
//!
//! Шейпинга нет: один символ — один глиф, без кернинга и лигатур (ограничение fontdue;
//! см. ADR-8 о переходе на cosmic-text/parley). Единицы переноса — графемы.

use crate::fonts::{FontId, Fonts};
use hey_core::{Color, Point};
use hey_draw::{Glyph, GlyphRun};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub struct TextStyle {
    /// Fallback-цепочка: первый шрифт, содержащий символ.
    pub chain: Vec<FontId>,
    pub size_px: f32,
    /// Межстрочный интервал в px; `None` — из метрик первого шрифта.
    pub line_height: Option<f32>,
    pub align: Align,
    /// Ширина контейнера; `None` — без ограничения.
    pub max_width: Option<f32>,
    /// Переносить по словам (иначе только по `\n`).
    pub wrap: bool,
    pub max_lines: Option<usize>,
    /// При обрезке по `max_lines`/ширине заменять хвост на «…».
    pub ellipsis: bool,
}

impl TextStyle {
    pub fn new(chain: Vec<FontId>, size_px: f32) -> Self {
        Self { chain, size_px, line_height: None, align: Align::Left, max_width: None, wrap: true, max_lines: None, ellipsis: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub font: FontId,
    pub id: u16,
    /// X пера относительно левого края раскладки.
    pub x: f32,
    pub advance: f32,
    /// Байтовое смещение символа в исходном тексте (для каретки и hit-test).
    pub byte: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub glyphs: Vec<PlacedGlyph>,
    pub width: f32,
    /// Y базовой линии относительно верха раскладки.
    pub baseline: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub lines: Vec<Line>,
    pub width: f32,
    pub height: f32,
    size_px: f32,
}

struct Unit {
    glyphs: Vec<PlacedGlyph>,
    w: f32,
    ws: bool,
}

fn unit(fonts: &Fonts, chain: &[FontId], size: f32, s: &str, base: usize) -> Unit {
    let mut glyphs = Vec::new();
    let mut w = 0.0;
    for (i, ch) in s.char_indices() {
        let Some((font, id)) = fonts.pick(chain, ch) else { continue };
        let advance = fonts.advance(font, id, size);
        // Комбинируемые символы внутри графемы не двигают перо.
        let advance = if i == 0 { advance } else { 0.0 };
        glyphs.push(PlacedGlyph { font, id, x: w, advance, byte: base + i });
        w += advance;
    }
    Unit { glyphs, w, ws: s.chars().all(char::is_whitespace) }
}

fn line_width(units: &[Unit]) -> f32 {
    units.iter().map(|u| u.w).sum()
}

fn trim_end(units: &mut Vec<Unit>) {
    while units.last().is_some_and(|u| u.ws) {
        units.pop();
    }
}

pub fn layout(fonts: &Fonts, text: &str, style: &TextStyle) -> Layout {
    let size = style.size_px;
    let limit = style.max_width.filter(|_| style.wrap).unwrap_or(f32::INFINITY);
    let mut lines: Vec<Vec<Unit>> = Vec::new();

    let mut base = 0;
    for para in text.split('\n') {
        let mut cur: Vec<Unit> = Vec::new();
        let mut curw = 0.0;
        for (wi, word) in para.split_word_bound_indices() {
            let units: Vec<Unit> = word.grapheme_indices(true).map(|(gi, g)| unit(fonts, &style.chain, size, g, base + wi + gi)).collect();
            let ww = line_width(&units);
            let ws = units.iter().all(|u| u.ws);
            if !ws && curw + ww > limit && !cur.is_empty() {
                trim_end(&mut cur);
                lines.push(std::mem::take(&mut cur));
                curw = 0.0;
            }
            if !ws && ww > limit {
                // Слово длиннее строки — режем по графемам.
                for u in units {
                    if curw + u.w > limit && !cur.is_empty() {
                        lines.push(std::mem::take(&mut cur));
                        curw = 0.0;
                    }
                    curw += u.w;
                    cur.push(u);
                }
            } else {
                curw += ww;
                cur.extend(units);
            }
        }
        trim_end(&mut cur);
        lines.push(cur);
        base += para.len() + 1;
    }

    // Обрезка и многоточие.
    let width_limit = style.max_width.unwrap_or(f32::INFINITY);
    let mut truncated = false;
    if let Some(n) = style.max_lines.filter(|&n| n > 0) {
        if lines.len() > n {
            lines.truncate(n);
            truncated = true;
        }
    }
    if style.ellipsis {
        let last = lines.last_mut().expect("минимум одна строка");
        if truncated || line_width(last) > width_limit {
            trim_end(last);
            let tail = last.last().map_or(text.len(), |u| u.glyphs[0].byte + 1);
            let dots = unit(fonts, &style.chain, size, "…", tail);
            while !last.is_empty() && line_width(last) + dots.w > width_limit {
                last.pop();
            }
            last.push(dots);
        }
    }

    let first = style.chain.first().copied().unwrap_or(0);
    let m = if fonts.is_empty() {
        crate::fonts::LineMetrics { ascent: size * 0.8, descent: -size * 0.2, line_gap: 0.0 }
    } else {
        fonts.line_metrics(first, size)
    };
    let lh = style.line_height.unwrap_or_else(|| m.height());
    let content_w = lines.iter().map(|l| line_width(l)).fold(0.0, f32::max);
    let box_w = style.max_width.unwrap_or(content_w);

    let out: Vec<Line> = lines
        .into_iter()
        .enumerate()
        .map(|(i, units)| {
            let width = line_width(&units);
            let off = match style.align {
                Align::Left => 0.0,
                Align::Center => (box_w - width) / 2.0,
                Align::Right => box_w - width,
            };
            let mut pen = off;
            let mut glyphs = Vec::new();
            for u in units {
                for mut g in u.glyphs {
                    g.x += pen;
                    glyphs.push(g);
                }
                pen += u.w;
            }
            let baseline = i as f32 * lh + (lh - m.height()) / 2.0 + m.line_gap / 2.0 + m.ascent;
            Line { glyphs, width, baseline }
        })
        .collect();

    Layout { height: out.len() as f32 * lh, width: content_w, lines: out, size_px: size }
}

impl Layout {
    /// Команды отрисовки: серии подряд идущих глифов одного шрифта.
    /// `origin` — левый верхний угол раскладки в физических пикселях.
    pub fn to_runs(&self, origin: Point, color: Color) -> Vec<GlyphRun> {
        let mut runs: Vec<GlyphRun> = Vec::new();
        for line in &self.lines {
            let mut cur: Option<GlyphRun> = None;
            for g in &line.glyphs {
                let glyph = Glyph { id: g.id as u32, x: origin.x + g.x, y: origin.y + line.baseline };
                match &mut cur {
                    Some(r) if r.font == g.font => r.glyphs.push(glyph),
                    _ => {
                        runs.extend(cur.take());
                        cur = Some(GlyphRun { font: g.font, size_px: self.size_px, color, glyphs: vec![glyph] });
                    }
                }
            }
            runs.extend(cur);
        }
        runs
    }
}

/// Ширина однострочного текста без переноса.
pub fn measure(fonts: &Fonts, text: &str, chain: &[FontId], size_px: f32) -> f32 {
    let mut st = TextStyle::new(chain.to_vec(), size_px);
    st.wrap = false;
    layout(fonts, text, &st).width
}
