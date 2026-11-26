// Copyright (c) Vitaly Golik (mjojo)

//! Text layout: measuring, word/grapheme wrapping, alignment, multiline, "…".
//!
//! Bidi: lines are reordered visually (UAX #9, levels from `unicode-bidi`); no glyph mirroring yet.
//!
//! Shaping: words are shaped with harfrust (kerning, ligatures, Arabic joining) per font and
//! direction run; fontdue only rasterizes by glyph id (see ADR-8). Wrapping units are graphemes.

use crate::fonts::{FontId, Fonts};
use hey_core::{Color, Point};
use hey_draw::{Glyph, GlyphRun};
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    /// Leading edge of the paragraph: left for LTR, right for RTL.
    Start,
    /// Trailing edge of the paragraph.
    End,
}

#[derive(Clone, Debug)]
pub struct TextStyle {
    /// Fallback chain: the first font that contains the character.
    pub chain: Vec<FontId>,
    pub size_px: f32,
    /// Line spacing in px; `None` — from the first font's metrics.
    pub line_height: Option<f32>,
    pub align: Align,
    /// Container width; `None` — unlimited.
    pub max_width: Option<f32>,
    /// Wrap by words (otherwise only at `\n`).
    pub wrap: bool,
    pub max_lines: Option<usize>,
    /// When truncating by `max_lines`/width, replace the tail with "…".
    pub ellipsis: bool,
    /// Base direction: `None` — detected per paragraph from the first strong character.
    pub rtl: Option<bool>,
}

impl TextStyle {
    pub fn new(chain: Vec<FontId>, size_px: f32) -> Self {
        Self {
            chain,
            size_px,
            line_height: None,
            align: Align::Left,
            max_width: None,
            wrap: true,
            max_lines: None,
            ellipsis: false,
            rtl: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub font: FontId,
    pub id: u16,
    /// Pen X relative to the left edge of the layout.
    pub x: f32,
    pub advance: f32,
    /// Byte offset of the character in the source text (for caret and hit-test).
    pub byte: usize,
    /// The glyph belongs to a right-to-left run.
    pub rtl: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub glyphs: Vec<PlacedGlyph>,
    pub width: f32,
    /// Baseline Y relative to the top of the layout.
    pub baseline: f32,
    /// The paragraph of this line is right-to-left.
    pub rtl: bool,
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
    /// Byte offset of the grapheme in the source text.
    byte: usize,
}

fn is_bidi_control(ch: char) -> bool {
    matches!(ch, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// A run of characters shaped together: one font, one direction.
struct Run {
    font: FontId,
    rtl: bool,
    chars: Vec<(usize, char)>,
}

/// Splits `chars` into runs by fallback font and by strong direction.
/// The font and direction stick to neutral characters, marks and joiners.
fn split_runs(fonts: &Fonts, chain: &[FontId], chars: &[(usize, char)]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for &(byte, ch) in chars {
        let rtl_class = match unicode_bidi::bidi_class(ch) {
            unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => Some(true),
            unicode_bidi::BidiClass::L => Some(false),
            _ => None,
        };
        if let Some(r) = runs.last_mut() {
            // Marks and joiners stay with their base; other chars take the first chain font.
            let attaches = matches!(unicode_bidi::bidi_class(ch), unicode_bidi::BidiClass::NSM | unicode_bidi::BidiClass::BN);
            let same_font =
                fonts.glyph_index(r.font, ch).is_some() && (attaches || fonts.pick(chain, ch).is_some_and(|(f, _)| f == r.font));
            if same_font && rtl_class.is_none_or(|c| c == r.rtl) {
                r.chars.push((byte, ch));
                continue;
            }
        }
        let Some((font, _)) = fonts.pick(chain, ch) else { continue };
        let rtl = rtl_class.unwrap_or_else(|| runs.last().is_some_and(|r| r.rtl));
        runs.push(Run { font, rtl, chars: vec![(byte, ch)] });
    }
    runs
}

/// Maximal runs of whitespace / non-whitespace with their byte offsets.
fn chunks(s: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    std::iter::from_fn(move || {
        let (i, c) = it.next()?;
        let ws = c.is_whitespace();
        let mut end = i + c.len_utf8();
        while let Some(&(j, d)) = it.peek() {
            if d.is_whitespace() != ws {
                break;
            }
            end = j + d.len_utf8();
            it.next();
        }
        let from = std::mem::replace(&mut start, end);
        Some((from, &s[from..end]))
    })
}

/// Wrapping units (one per grapheme) for `word`, shaped as a whole so that kerning,
/// ligatures and contextual forms work. A ligature puts its glyph on the first grapheme;
/// the following graphemes become empty zero-width units.
fn word_units(fonts: &Fonts, chain: &[FontId], size: f32, word: &str, base: usize) -> Vec<Unit> {
    let mut units: Vec<Unit> = word
        .grapheme_indices(true)
        .map(|(gi, g)| Unit { glyphs: Vec::new(), w: 0.0, ws: g.chars().all(char::is_whitespace), byte: base + gi })
        .collect();
    if units.is_empty() {
        return units;
    }
    // Invisible bidi formatting characters take part in reordering but produce no glyph.
    let chars: Vec<(usize, char)> = word.char_indices().filter(|&(_, c)| !is_bidi_control(c)).map(|(i, c)| (base + i, c)).collect();
    for run in split_runs(fonts, chain, &chars) {
        let shaped = fonts.shape(run.font, &run.chars, run.rtl, size).unwrap_or_else(|| {
            // Fallback without shaping: one glyph per character.
            run.chars
                .iter()
                .map(|&(byte, ch)| {
                    let id = fonts.glyph_index(run.font, ch).unwrap_or(0);
                    crate::fonts::ShapedGlyph { id, cluster: byte, advance: fonts.advance(run.font, id, size), x_offset: 0.0 }
                })
                .collect()
        });
        for g in shaped {
            let ui = units.partition_point(|u| u.byte <= g.cluster).saturating_sub(1);
            let u = &mut units[ui];
            u.glyphs.push(PlacedGlyph { font: run.font, id: g.id, x: u.w + g.x_offset, advance: g.advance, byte: g.cluster, rtl: false });
            u.w += g.advance;
        }
    }
    units
}

fn line_width(units: &[Unit]) -> f32 {
    units.iter().map(|u| u.w).sum()
}

fn trim_end(units: &mut Vec<Unit>) {
    while units.last().is_some_and(|u| u.ws) {
        units.pop();
    }
}

/// UAX #9 rule L2: from the highest level down to the lowest odd one, reverse every maximal run at that level or above.
fn reorder<T>(items: &mut [T], levels: &[Level]) {
    let max = levels.iter().map(|l| l.number()).max().unwrap_or(0);
    let min_odd = levels.iter().map(|l| l.number()).filter(|n| n % 2 == 1).min();
    let Some(min_odd) = min_odd else { return };
    for lvl in (min_odd..=max).rev() {
        let mut i = 0;
        while i < levels.len() {
            if levels[i].number() >= lvl {
                let mut j = i;
                while j < levels.len() && levels[j].number() >= lvl {
                    j += 1;
                }
                items[i..j].reverse();
                i = j;
            } else {
                i += 1;
            }
        }
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
        // Shape whole whitespace-delimited chunks so that features may span punctuation
        // (`->`); wrapping still happens on UAX #29 word boundaries.
        let mut shaped = chunks(para).flat_map(|(ci, chunk)| word_units(fonts, &style.chain, size, chunk, base + ci)).peekable();
        for (wi, word) in para.split_word_bound_indices() {
            let end = base + wi + word.len();
            let mut units = Vec::new();
            while let Some(u) = shaped.next_if(|u| u.byte < end) {
                units.push(u);
            }
            let ww = line_width(&units);
            let ws = units.iter().all(|u| u.ws);
            if !ws && curw + ww > limit && !cur.is_empty() {
                trim_end(&mut cur);
                lines.push(std::mem::take(&mut cur));
                curw = 0.0;
            }
            if !ws && ww > limit {
                // The word is longer than the line — split by graphemes.
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

    // Truncation and ellipsis.
    let width_limit = style.max_width.unwrap_or(f32::INFINITY);
    let mut truncated = false;
    if let Some(n) = style.max_lines.filter(|&n| n > 0) {
        if lines.len() > n {
            lines.truncate(n);
            truncated = true;
        }
    }
    if style.ellipsis {
        let last = lines.last_mut().expect("at least one line");
        if truncated || line_width(last) > width_limit {
            trim_end(last);
            let tail = last.last().map_or(text.len(), |u| u.byte + 1);
            let dots = word_units(fonts, &style.chain, size, "…", tail);
            let dots_w = line_width(&dots);
            while !last.is_empty() && line_width(last) + dots_w > width_limit {
                last.pop();
            }
            last.extend(dots);
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

    let base_level = style.rtl.map(|r| if r { Level::rtl() } else { Level::ltr() });
    let info = (base_level.is_some() || !text.is_ascii()).then(|| BidiInfo::new(text, base_level));
    let mut last_rtl = style.rtl.unwrap_or(false);

    let out: Vec<Line> = lines
        .into_iter()
        .enumerate()
        .map(|(i, mut units)| {
            let width = line_width(&units);
            let mut line_rtl = last_rtl;
            if let Some(info) = &info {
                let first = units.first().map(|u| u.byte);
                if let Some(para) = first.and_then(|b| info.paragraphs.iter().find(|p| p.range.contains(&b))) {
                    line_rtl = para.level.is_rtl();
                }
                let para_level = if line_rtl { Level::rtl() } else { Level::ltr() };
                let levels: Vec<Level> = units.iter().map(|u| info.levels.get(u.byte).copied().unwrap_or(para_level)).collect();
                for (u, l) in units.iter_mut().zip(&levels) {
                    u.glyphs.iter_mut().for_each(|g| g.rtl = l.is_rtl());
                }
                reorder(&mut units, &levels);
            }
            last_rtl = line_rtl;
            let off = match (style.align, line_rtl) {
                (Align::Left, _) | (Align::Start, false) | (Align::End, true) => 0.0,
                (Align::Center, _) => (box_w - width) / 2.0,
                (Align::Right, _) | (Align::Start, true) | (Align::End, false) => box_w - width,
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
            Line { glyphs, width, baseline, rtl: line_rtl }
        })
        .collect();

    Layout { height: out.len() as f32 * lh, width: content_w, lines: out, size_px: size }
}

impl Layout {
    /// X of the caret at logical byte offset `byte` on `line`. `None` for a line without glyphs.
    pub fn caret_x(&self, line: usize, byte: usize) -> Option<f32> {
        let ln = self.lines.get(line)?;
        if let Some(g) = ln.glyphs.iter().filter(|g| g.byte >= byte).min_by_key(|g| g.byte) {
            return Some(if g.rtl { g.x + g.advance } else { g.x });
        }
        // Past the logical end of the line: trailing edge of the logically last glyph.
        let g = ln.glyphs.iter().filter(|g| g.advance > 0.0 || g.byte > 0).max_by_key(|g| g.byte)?;
        Some(if g.rtl { g.x } else { g.x + g.advance })
    }

    /// Logical byte offset of the caret position nearest to `x` on `line`. `None` for a line without glyphs.
    pub fn byte_at_x(&self, text: &str, line: usize, x: f32) -> Option<usize> {
        let ln = self.lines.get(line)?;
        let after = |g: &PlacedGlyph| g.byte + text[g.byte..].graphemes(true).next().map_or(0, str::len);
        for g in ln.glyphs.iter().filter(|g| g.advance > 0.0) {
            if x < g.x + g.advance / 2.0 {
                return Some(if g.rtl { after(g) } else { g.byte });
            }
        }
        let g = ln.glyphs.iter().rfind(|g| g.advance > 0.0).or(ln.glyphs.last())?;
        Some(if g.rtl { g.byte } else { after(g) })
    }

    /// Draw commands: runs of consecutive glyphs from the same font.
    /// `origin` is the top-left corner of the layout in physical pixels.
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

/// Width of single-line text without wrapping.
pub fn measure(fonts: &Fonts, text: &str, chain: &[FontId], size_px: f32) -> f32 {
    let mut st = TextStyle::new(chain.to_vec(), size_px);
    st.wrap = false;
    layout(fonts, text, &st).width
}
