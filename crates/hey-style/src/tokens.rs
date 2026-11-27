// Copyright (c) Vitaly Golik (mjojo)

//! Design tokens as a contract: validate a `themes.json` and export it to CSS custom properties
//! and to a Design Tokens (DTCG) file that Figma plugins such as Tokens Studio import.

use crate::theme::KNOWN;
use crate::Theme;
use hey_core::Color;
use serde_json::{json, Map, Value};
use std::fmt;

/// One problem found in a token file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenIssue {
    /// Theme id, or empty for file-level problems.
    pub theme: String,
    /// Token path, e.g. `color.textMuted` or `motion.fastMs`.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for TokenIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.theme.is_empty() {
            write!(f, "{}: {}", self.path, self.message)
        } else {
            write!(f, "{}: {}: {}", self.theme, self.path, self.message)
        }
    }
}

struct Sink<'a> {
    theme: &'a str,
    out: &'a mut Vec<TokenIssue>,
}

impl Sink<'_> {
    fn add(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.out.push(TokenIssue { theme: self.theme.into(), path: path.into(), message: message.into() });
    }
    fn color(&mut self, v: Option<&Value>, path: &str) {
        match v {
            None => self.add(path, "missing"),
            Some(v) if v.as_str().and_then(Color::parse).is_some() => {}
            Some(_) => self.add(path, "not a colour (expected #RRGGBB or #RRGGBBAA)"),
        }
    }
    fn range(&mut self, v: Option<&Value>, path: &str, lo: f64, hi: f64) -> Option<f64> {
        match v.and_then(Value::as_f64) {
            None => {
                self.add(path, "missing or not a number");
                None
            }
            Some(x) if x < lo || x > hi => {
                self.add(path, format!("{x} is outside {lo}..={hi}"));
                None
            }
            Some(x) => Some(x),
        }
    }
}

fn check_theme(id: &str, t: &Value, out: &mut Vec<TokenIssue>) {
    let before = out.len();
    let mut s = Sink { theme: id, out };
    match t.get("color") {
        Some(c) => {
            for k in KNOWN {
                s.color(c.get(k), &format!("color.{k}"));
            }
        }
        None => s.add("color", "missing"),
    }
    for k in ["small", "medium", "large"] {
        s.range(t.pointer(&format!("/chamfer/{k}")), &format!("chamfer.{k}"), 0.0, 64.0);
    }
    s.range(t.pointer("/focus/width"), "focus.width", 1.0, 8.0);
    s.range(t.pointer("/focus/offset"), "focus.offset", 0.0, 16.0);
    let fast = s.range(t.pointer("/motion/fastMs"), "motion.fastMs", 0.0, 1000.0);
    let base = s.range(t.pointer("/motion/baseMs"), "motion.baseMs", 0.0, 1000.0);
    let win = s.range(t.pointer("/motion/windowMs"), "motion.windowMs", 0.0, 1000.0);
    if let (Some(f), Some(b), Some(w)) = (fast, base, win) {
        if !(f <= b && b <= w) {
            s.add("motion", "durations must satisfy fastMs <= baseMs <= windowMs");
        }
    }
    for k in ["ui", "mono"] {
        if t.pointer(&format!("/font/{k}")).and_then(Value::as_str).is_none_or(|v| v.trim().is_empty()) {
            s.add(format!("font.{k}"), "missing or empty");
        }
    }
    if let Some(cs) = t.get("controlStyle") {
        if !matches!(cs.as_str(), Some("flat" | "outline" | "bevel")) {
            s.add("controlStyle", "must be flat, outline or bevel");
        }
    }
    s.color(t.pointer("/terminal/background"), "terminal.background");
    s.color(t.pointer("/terminal/cursor"), "terminal.cursor");
    match t.pointer("/terminal/ansi").and_then(Value::as_array) {
        Some(a) if a.len() == 16 => {
            for (i, v) in a.iter().enumerate() {
                s.color(Some(v), &format!("terminal.ansi[{i}]"));
            }
        }
        Some(a) => s.add("terminal.ansi", format!("needs 16 colours, found {}", a.len())),
        None => s.add("terminal.ansi", "missing"),
    }
    // Contrast only makes sense once the structure is sound.
    if out.len() == before {
        match Theme::from_value(id, t) {
            Ok(theme) => {
                for c in theme.audit_contrast() {
                    out.push(TokenIssue {
                        theme: id.into(),
                        path: format!("contrast.{}/{}", c.foreground, c.background),
                        message: format!("ratio {:.2}:1 is below the required {:.1}:1", c.ratio, c.required),
                    });
                }
            }
            Err(e) => out.push(TokenIssue { theme: id.into(), path: "theme".into(), message: e.to_string() }),
        }
    }
}

/// Checks a `themes.json` document: every colour token present and valid, numeric ranges,
/// motion order, terminal palette, and WCAG contrast of the pairs widgets draw.
/// An empty result means the file honours the contract.
pub fn validate_tokens(json: &str) -> Vec<TokenIssue> {
    let mut out = Vec::new();
    let root: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => {
            out.push(TokenIssue { theme: String::new(), path: "json".into(), message: e.to_string() });
            return out;
        }
    };
    match root.get("themes").and_then(Value::as_object) {
        Some(m) if !m.is_empty() => {
            for (id, t) in m {
                check_theme(id, t, &mut out);
            }
        }
        _ => out.push(TokenIssue { theme: String::new(), path: "themes".into(), message: "missing or empty".into() }),
    }
    out
}

fn hex(c: Color) -> String {
    if c.a == 255 {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
    }
}

/// `textMuted` -> `text-muted`.
fn kebab(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            o.push('-');
            o.push(ch.to_ascii_lowercase());
        } else {
            o.push(ch);
        }
    }
    o
}

fn colors(t: &Theme) -> Vec<(String, Color)> {
    let p = &t.palette;
    let named = [
        ("desktop", p.desktop),
        ("desktopDeep", p.desktop_deep),
        ("panel", p.panel),
        ("document", p.document),
        ("control", p.control),
        ("controlHover", p.control_hover),
        ("line", p.line),
        ("tabInactive", p.tab_inactive),
        ("accent", p.accent),
        ("accentDark", p.accent_dark),
        ("onAccent", p.on_accent),
        ("focus", p.focus),
        ("text", p.text),
        ("textMuted", p.text_muted),
        ("textDisabled", p.text_disabled),
        ("success", p.success),
        ("warning", p.warning),
        ("error", p.error),
        ("media", p.media),
        ("selection", p.selection),
        ("onSelection", p.on_selection),
    ];
    let mut v: Vec<(String, Color)> = named.iter().map(|(k, c)| (k.to_string(), *c)).collect();
    let mut extra: Vec<_> = p.extra.iter().map(|(k, c)| (k.clone(), *c)).collect();
    extra.sort_by(|a, b| a.0.cmp(&b.0));
    v.extend(extra);
    v
}

impl Theme {
    /// CSS custom properties for this theme inside a `[data-theme="id"]` rule.
    pub fn to_css(&self) -> String {
        let mut s = format!("[data-theme=\"{}\"] {{\n", self.id);
        let mut line = |k: String, v: String| s.push_str(&format!("  --hey-{k}: {v};\n"));
        for (k, c) in colors(self) {
            line(format!("color-{}", kebab(&k)), hex(c));
        }
        for (k, v) in ["small", "medium", "large"].iter().zip(self.chamfer) {
            line(format!("chamfer-{k}"), format!("{v}px"));
        }
        line("focus-width".into(), format!("{}px", self.focus_width));
        line("focus-offset".into(), format!("{}px", self.focus_offset));
        line("motion-fast".into(), format!("{}ms", self.fast_ms));
        line("motion-base".into(), format!("{}ms", self.base_ms));
        line("motion-window".into(), format!("{}ms", self.window_ms));
        line("font-ui".into(), self.font_ui.join(", "));
        line("font-mono".into(), self.font_mono.clone());
        line("terminal-background".into(), hex(self.terminal_bg));
        line("terminal-cursor".into(), hex(self.terminal_cursor));
        for (i, c) in self.ansi.iter().enumerate() {
            line(format!("ansi-{i}"), hex(*c));
        }
        s.push_str("}\n");
        s
    }

    /// Design Tokens Community Group JSON (`$type` / `$value`), importable into Figma via Tokens Studio.
    pub fn to_design_tokens(&self) -> Value {
        let tok = |ty: &str, v: Value| json!({ "$type": ty, "$value": v });
        let mut color = Map::new();
        for (k, c) in colors(self) {
            color.insert(k, tok("color", hex(c).into()));
        }
        let mut ansi = Map::new();
        for (i, c) in self.ansi.iter().enumerate() {
            ansi.insert(i.to_string(), tok("color", hex(*c).into()));
        }
        json!({
            "color": color,
            "chamfer": {
                "small": tok("dimension", format!("{}px", self.chamfer[0]).into()),
                "medium": tok("dimension", format!("{}px", self.chamfer[1]).into()),
                "large": tok("dimension", format!("{}px", self.chamfer[2]).into()),
            },
            "focus": {
                "width": tok("dimension", format!("{}px", self.focus_width).into()),
                "offset": tok("dimension", format!("{}px", self.focus_offset).into()),
            },
            "motion": {
                "fast": tok("duration", format!("{}ms", self.fast_ms).into()),
                "base": tok("duration", format!("{}ms", self.base_ms).into()),
                "window": tok("duration", format!("{}ms", self.window_ms).into()),
            },
            "font": {
                "ui": tok("fontFamily", Value::from(self.font_ui.clone())),
                "mono": tok("fontFamily", self.font_mono.clone().into()),
            },
            "terminal": {
                "background": tok("color", hex(self.terminal_bg).into()),
                "cursor": tok("color", hex(self.terminal_cursor).into()),
                "ansi": ansi,
            },
        })
    }
}

fn all_themes(json: &str) -> Result<Vec<Theme>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let m = root.get("themes").and_then(Value::as_object).ok_or("missing \"themes\"")?;
    m.iter().map(|(id, t)| Theme::from_value(id, t).map_err(|e| format!("{id}: {e}"))).collect()
}

/// CSS for every theme in a `themes.json` document.
pub fn export_css(json: &str) -> Result<String, String> {
    Ok(all_themes(json)?.iter().map(Theme::to_css).collect::<Vec<_>>().join("\n"))
}

/// One DTCG document with a top-level group per theme id.
pub fn export_figma(json: &str) -> Result<String, String> {
    let mut m = Map::new();
    for t in all_themes(json)? {
        m.insert(t.id.clone(), t.to_design_tokens());
    }
    serde_json::to_string_pretty(&Value::Object(m)).map_err(|e| e.to_string())
}
