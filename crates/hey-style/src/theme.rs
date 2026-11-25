// Copyright (c) Vitaly Golik (mjojo)

use crate::{Easing, Variant, WidgetState};
use hey_core::Color;
use serde_json::Value;
use std::collections::HashMap;

const THEMES_JSON: &str = include_str!("../../../design/tokens/themes.json");

#[derive(Debug)]
pub enum ThemeError {
    Json(String),
    Missing(String),
    BadColor(String),
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::Missing(k) => write!(f, "missing field `{k}`"),
            Self::BadColor(k) => write!(f, "invalid color `{k}`"),
        }
    }
}
impl std::error::Error for ThemeError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    #[default]
    Normal,
    Compact,
}

impl Density {
    pub fn scale(self) -> f32 {
        match self {
            Density::Normal => 1.0,
            Density::Compact => 0.8,
        }
    }
}

/// Theme colors. Common tokens are fields; theme-specific ones
/// (`primaryFill`, `bevelLight`, …) live in `extra`.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub desktop: Color,
    pub desktop_deep: Color,
    pub panel: Color,
    pub document: Color,
    pub control: Color,
    pub control_hover: Color,
    pub line: Color,
    pub tab_inactive: Color,
    pub accent: Color,
    pub accent_dark: Color,
    pub on_accent: Color,
    pub focus: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_disabled: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub media: Color,
    pub selection: Color,
    pub on_selection: Color,
    pub extra: HashMap<String, Color>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    /// `flat` / `outline` / `bevel` — control style.
    pub control_style: String,
    pub palette: Palette,
    /// Bevel sizes (small, medium, large) in physical px at Normal density.
    pub chamfer: [f32; 3],
    pub focus_width: f32,
    pub focus_offset: f32,
    pub fast_ms: f32,
    pub base_ms: f32,
    pub window_ms: f32,
    pub easing: Easing,
    pub font_ui: Vec<String>,
    pub font_mono: String,
    pub terminal_bg: Color,
    pub terminal_cursor: Color,
    pub ansi: [Color; 16],
    pub density: Density,
}

fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value, ThemeError> {
    v.get(k).ok_or_else(|| ThemeError::Missing(k.into()))
}
fn color(v: &Value, k: &str) -> Result<Color, ThemeError> {
    get(v, k)?.as_str().and_then(Color::parse).ok_or_else(|| ThemeError::BadColor(k.into()))
}
fn num(v: &Value, k: &str) -> Result<f32, ThemeError> {
    get(v, k)?.as_f64().map(|x| x as f32).ok_or_else(|| ThemeError::Missing(k.into()))
}

const KNOWN: [&str; 21] = [
    "desktop",
    "desktopDeep",
    "panel",
    "document",
    "control",
    "controlHover",
    "line",
    "tabInactive",
    "accent",
    "accentDark",
    "onAccent",
    "focus",
    "text",
    "textMuted",
    "textDisabled",
    "success",
    "warning",
    "error",
    "media",
    "selection",
    "onSelection",
];

impl Theme {
    /// Parses a single theme (the `themes.<id>` object from `design/tokens/themes.json`).
    pub fn from_value(id: &str, t: &Value) -> Result<Theme, ThemeError> {
        let c = get(t, "color")?;
        let mut extra = HashMap::new();
        for (k, v) in c.as_object().into_iter().flatten() {
            if !KNOWN.contains(&k.as_str()) {
                if let Some(col) = v.as_str().and_then(Color::parse) {
                    extra.insert(k.clone(), col);
                }
            }
        }
        let palette = Palette {
            desktop: color(c, "desktop")?,
            desktop_deep: color(c, "desktopDeep")?,
            panel: color(c, "panel")?,
            document: color(c, "document")?,
            control: color(c, "control")?,
            control_hover: color(c, "controlHover")?,
            line: color(c, "line")?,
            tab_inactive: color(c, "tabInactive")?,
            accent: color(c, "accent")?,
            accent_dark: color(c, "accentDark")?,
            on_accent: color(c, "onAccent")?,
            focus: color(c, "focus")?,
            text: color(c, "text")?,
            text_muted: color(c, "textMuted")?,
            text_disabled: color(c, "textDisabled")?,
            success: color(c, "success")?,
            warning: color(c, "warning")?,
            error: color(c, "error")?,
            media: color(c, "media")?,
            selection: color(c, "selection")?,
            on_selection: color(c, "onSelection")?,
            extra,
        };
        let ch = get(t, "chamfer")?;
        let fo = get(t, "focus")?;
        let mo = get(t, "motion")?;
        let fnt = get(t, "font")?;
        let term = get(t, "terminal")?;
        let arr = get(term, "ansi")?.as_array().ok_or_else(|| ThemeError::Missing("ansi".into()))?;
        if arr.len() != 16 {
            return Err(ThemeError::Missing("ansi[16]".into()));
        }
        let mut ansi = [Color::TRANSPARENT; 16];
        for (i, v) in arr.iter().enumerate() {
            ansi[i] = v.as_str().and_then(Color::parse).ok_or_else(|| ThemeError::BadColor(format!("ansi[{i}]")))?;
        }
        Ok(Theme {
            id: id.into(),
            name: t.get("name").and_then(Value::as_str).unwrap_or(id).into(),
            control_style: t.get("controlStyle").and_then(Value::as_str).unwrap_or("flat").into(),
            palette,
            chamfer: [num(ch, "small")?, num(ch, "medium")?, num(ch, "large")?],
            focus_width: num(fo, "width")?,
            focus_offset: num(fo, "offset")?,
            fast_ms: num(mo, "fastMs")?,
            base_ms: num(mo, "baseMs")?,
            window_ms: num(mo, "windowMs")?,
            easing: mo.get("curve").and_then(Value::as_str).and_then(Easing::parse).unwrap_or(Easing::THEME),
            font_ui: get(fnt, "ui")?.as_str().unwrap_or("").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            font_mono: get(fnt, "mono")?.as_str().unwrap_or("").into(),
            terminal_bg: color(term, "background")?,
            terminal_cursor: color(term, "cursor")?,
            ansi,
            density: Density::Normal,
        })
    }

    /// Loads from JSON in the `themes.json` format (`{"themes": {id: …}}`).
    /// Basis for loading from disk and hot-reload.
    pub fn from_json(json: &str, id: &str) -> Result<Theme, ThemeError> {
        let root: Value = serde_json::from_str(json).map_err(|e| ThemeError::Json(e.to_string()))?;
        let t = root.get("themes").and_then(|t| t.get(id)).ok_or_else(|| ThemeError::Missing(id.into()))?;
        Self::from_value(id, t)
    }

    /// Built-in theme from `design/tokens/themes.json`: `industrial-dark`, `night-ops`, `zeta-remastered`.
    pub fn builtin(id: &str) -> Option<Theme> {
        Self::from_json(THEMES_JSON, id).ok()
    }
    /// Main dark theme (Industrial Dark; desktop background #0B0C10 is `desktop_deep`).
    pub fn dark() -> Theme {
        Self::builtin("industrial-dark").expect("built-in theme is valid")
    }
    /// Light theme based on `lightBg` (#F4F1EA) from tokens.json; the accent is kept.
    pub fn light() -> Theme {
        let mut t = Self::dark();
        let p = &mut t.palette;
        p.desktop = Color::hex(0xE6E2D8);
        p.desktop_deep = Color::hex(0xD9D4C8);
        p.panel = Color::hex(0xF4F1EA);
        p.document = Color::hex(0xFFFFFF);
        p.control = Color::hex(0xE9E5DB);
        p.control_hover = Color::hex(0xDDD8CB);
        p.line = Color::hex(0xCFC9BA);
        p.tab_inactive = Color::hex(0xCFC9BA);
        p.text = Color::hex(0x14161B);
        p.text_muted = Color::hex(0x5B6270);
        p.text_disabled = Color::hex(0x9AA0AB);
        p.accent_dark = Color::hex(0x8A5200);
        p.error = Color::hex(0xD81B4A);
        p.success = Color::hex(0x0B9B45);
        p.focus = Color::hex(0x007C8C);
        t.id = "light".into();
        t.name = "HeOS Light".into();
        t
    }
    pub fn compact(mut self) -> Theme {
        self.density = Density::Compact;
        self
    }

    pub fn chamfer_small(&self) -> f32 {
        self.chamfer[0] * self.density.scale()
    }
    pub fn chamfer_medium(&self) -> f32 {
        self.chamfer[1] * self.density.scale()
    }
    pub fn chamfer_large(&self) -> f32 {
        self.chamfer[2] * self.density.scale()
    }
    /// Base padding/height, scaled by density.
    pub fn space(&self, px: f32) -> f32 {
        (px * self.density.scale()).round()
    }

    /// Button background for a variant and state.
    pub fn button_bg(&self, v: Variant, s: WidgetState) -> Color {
        let p = &self.palette;
        if s.disabled() {
            return p.control.lerp(p.panel, 0.5);
        }
        let white = Color::rgb(255, 255, 255);
        let black = Color::rgb(0, 0, 0);
        let (base, hover, pressed) = match v {
            Variant::Primary => (p.accent, p.accent.lerp(white, 0.2), p.accent_dark),
            Variant::Danger => (p.error, p.error.lerp(white, 0.2), p.error.lerp(black, 0.3)),
            Variant::Secondary => (p.control, p.control_hover, p.control.lerp(black, 0.25)),
        };
        if s.pressed() {
            pressed
        } else if s.hovered() {
            hover
        } else {
            base
        }
    }
    /// Text color on a button.
    pub fn button_fg(&self, v: Variant, s: WidgetState) -> Color {
        let p = &self.palette;
        if s.disabled() {
            return p.text_disabled;
        }
        match v {
            Variant::Primary => p.on_accent,
            Variant::Danger => Color::rgb(255, 255, 255),
            Variant::Secondary => p.text,
        }
    }
}
