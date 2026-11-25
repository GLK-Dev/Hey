// Copyright (c) Vitaly Golik (mjojo)

//! hey-text: fonts, text layout and editing (independent of the UI).

/// Bundled default font: Onest variable font (SIL OFL 1.1, see `assets/OFL.txt`).
pub const DEFAULT_FONT: &[u8] = include_bytes!("../assets/Onest-VF.ttf");

mod edit;
mod fonts;
mod layout;

pub use edit::{Clipboard, MemoryClipboard, TextEditState};
pub use fonts::{FontError, FontId, Fonts, GlyphBitmap, LineMetrics};
pub use layout::{layout, measure, Align, Layout, Line, PlacedGlyph, TextStyle};

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_font_matches_design_original() {
        let orig = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/fonts/onest/Onest[wght].ttf");
        if let Ok(bytes) = std::fs::read(orig) {
            assert!(bytes == super::DEFAULT_FONT, "crates/hey-text/assets/Onest-VF.ttf is out of sync with design/");
        }
    }
}
