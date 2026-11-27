// Copyright (c) Vitaly Golik (mjojo)

//! Theme accessibility audit: WCAG 2.x contrast of the colour pairs widgets actually draw.

use crate::Theme;
use hey_core::Color;

/// One colour pair that falls below its required contrast ratio.
#[derive(Clone, Debug, PartialEq)]
pub struct ContrastIssue {
    /// Foreground token, e.g. `text_muted`.
    pub foreground: &'static str,
    /// Background token, e.g. `panel`.
    pub background: &'static str,
    /// Measured contrast ratio (1.0..=21.0).
    pub ratio: f32,
    /// Ratio the pair must reach (4.5 for text, 3.0 for UI components).
    pub required: f32,
}

/// WCAG relative luminance of an sRGB colour (alpha ignored).
pub fn relative_luminance(c: Color) -> f32 {
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two colours, from 1.0 (identical) to 21.0 (black on white).
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

impl Theme {
    /// Checks the pairs widgets draw against WCAG AA and returns every failure.
    /// Disabled text is exempt, as WCAG allows. An empty result means the theme passes.
    pub fn audit_contrast(&self) -> Vec<ContrastIssue> {
        let p = &self.palette;
        let text: [(&str, Color, &str, Color); 7] = [
            ("text", p.text, "panel", p.panel),
            ("text", p.text, "document", p.document),
            ("text", p.text, "control", p.control),
            ("text", p.text, "control_hover", p.control_hover),
            ("text_muted", p.text_muted, "panel", p.panel),
            ("on_accent", p.on_accent, "accent", p.accent),
            ("on_selection", p.on_selection, "selection", p.selection),
        ];
        let ui: [(&str, Color, &str, Color); 2] = [("focus", p.focus, "panel", p.panel), ("accent", p.accent, "panel", p.panel)];
        let mut out = Vec::new();
        for (required, pairs) in [(4.5, &text[..]), (3.0, &ui[..])] {
            for &(fg, fc, bg, bc) in pairs {
                let ratio = contrast_ratio(fc, bc);
                if ratio < required {
                    out.push(ContrastIssue { foreground: fg, background: bg, ratio, required });
                }
            }
        }
        out
    }
}
