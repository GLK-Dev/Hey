// Copyright (c) Vitaly Golik (mjojo)

//! hey-text: шрифты, раскладка и редактирование текста (без привязки к UI).

mod edit;
mod fonts;
mod layout;

pub use edit::{Clipboard, MemoryClipboard, TextEditState};
pub use fonts::{FontError, FontId, Fonts, GlyphBitmap, LineMetrics};
pub use layout::{layout, measure, Align, Layout, Line, PlacedGlyph, TextStyle};
