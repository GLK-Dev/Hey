// Copyright (c) Vitaly Golik (mjojo)

use hey_core::Color;
use hey_style::*;

#[test]
fn builtin_themes_load() {
    for id in ["industrial-dark", "night-ops", "zeta-remastered"] {
        let t = Theme::builtin(id).unwrap_or_else(|| panic!("{id}"));
        assert!(t.fast_ms > 0.0);
    }
    let d = Theme::dark();
    assert_eq!(d.palette.desktop_deep, Color::hex(0x0B0C10));
    assert_eq!(d.palette.accent, Color::hex(0xF5A623));
    assert_eq!(d.easing, Easing::THEME);
    assert!(Theme::builtin("night-ops").unwrap().palette.extra.contains_key("primaryFill"));
    assert!(Theme::builtin("nope").is_none());
}

#[test]
fn bad_json_reports_errors() {
    assert!(matches!(Theme::from_json("{", "x"), Err(ThemeError::Json(_))));
    assert!(matches!(Theme::from_json(r#"{"themes":{}}"#, "x"), Err(ThemeError::Missing(_))));
}

#[test]
fn light_and_compact() {
    assert_eq!(Theme::light().palette.panel, Color::hex(0xF4F1EA));
    let c = Theme::dark().compact();
    assert!(c.chamfer_medium() < Theme::dark().chamfer_medium());
    assert_eq!(c.space(10.0), 8.0);
}

#[test]
fn widget_state_flags() {
    let mut s = WidgetState::NORMAL;
    assert!(s.is_normal());
    s.set(WidgetState::HOVERED, true);
    s = s | WidgetState::FOCUSED;
    assert!(s.hovered() && s.focused() && !s.pressed());
    s.set(WidgetState::HOVERED, false);
    assert!(!s.hovered() && s.focused());
}

#[test]
fn button_colors_follow_state() {
    let t = Theme::dark();
    let n = t.button_bg(Variant::Primary, WidgetState::NORMAL);
    let h = t.button_bg(Variant::Primary, WidgetState::HOVERED);
    let p = t.button_bg(Variant::Primary, WidgetState::HOVERED | WidgetState::PRESSED);
    assert_eq!(n, t.palette.accent);
    assert_ne!(n, h);
    assert_eq!(p, t.palette.accent_dark);
    assert_eq!(t.button_fg(Variant::Secondary, WidgetState::DISABLED), t.palette.text_disabled);
}

#[test]
fn easing_and_transition() {
    assert_eq!(Easing::parse("cubic-bezier(.2,.8,.2,1)"), Some(Easing::THEME));
    assert_eq!(Easing::parse("bogus"), None);
    let e = Easing::THEME;
    assert!(e.apply(0.0).abs() < 1e-3 && (e.apply(1.0) - 1.0).abs() < 1e-3);
    assert!(e.apply(0.3) > 0.3, "ease-out is faster than linear");

    let mut t = Transition::new(0.0f32, 100.0, Easing::Linear);
    assert!(!t.is_running());
    t.set(10.0);
    t.tick(50.0);
    assert!((t.value() - 5.0).abs() < 1e-4 && t.is_running());
    t.set(0.0);
    assert!((t.value() - 5.0).abs() < 1e-4);
    t.tick(200.0);
    assert_eq!(t.value(), 0.0);
    assert!(!t.is_running());

    let mut c = Transition::new(Color::rgb(0, 0, 0), 100.0, Easing::Linear);
    c.set(Color::rgb(200, 100, 0));
    c.tick(50.0);
    assert_eq!(c.value(), Color::rgb(100, 50, 0));
}

#[test]
fn high_contrast_themes_meet_aaa_for_text() {
    for t in [Theme::high_contrast_dark(), Theme::high_contrast_light()] {
        let p = &t.palette;
        assert!(p.text.contrast(p.panel) >= 7.0, "{}: text on panel", t.id);
        assert!(p.text.contrast(p.document) >= 7.0, "{}: text on document", t.id);
        assert!(p.on_accent.contrast(p.accent) >= 7.0, "{}: text on accent", t.id);
        assert!(p.on_selection.contrast(p.selection) >= 7.0, "{}: text on selection", t.id);
        assert!(p.focus.contrast(p.panel) >= 7.0, "{}: focus ring", t.id);
        assert!(t.focus_width >= 3.0);
    }
}

#[test]
fn appearance_selects_theme_and_reduces_motion() {
    use hey_style::Appearance;
    let d = Theme::for_appearance(Appearance::default());
    assert_eq!(d.palette, Theme::dark().palette);
    assert!(d.base_ms > 0.0);
    let l = Theme::for_appearance(Appearance { dark: false, ..Appearance::default() });
    assert_eq!(l.palette, Theme::light().palette);
    let hc = Theme::for_appearance(Appearance { dark: false, high_contrast: true, reduce_motion: true, ..Appearance::default() });
    assert_eq!(hc.id, "high-contrast-light");
    assert_eq!((hc.fast_ms, hc.base_ms, hc.window_ms), (0.0, 0.0, 0.0));
}

#[test]
fn theme_from_file_reads_and_reports_errors() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes.json");
    let t = Theme::from_file(path, "industrial-dark").unwrap();
    assert_eq!(t.id, "industrial-dark");
    assert!(matches!(Theme::from_file("no/such/file.json", "x"), Err(ThemeError::Io(_))));
    assert!(matches!(Theme::from_file(path, "nope"), Err(ThemeError::Missing(_))));
}

#[test]
fn builtin_themes_match_chamfer_tokens() {
    assert_eq!(Theme::builtin("industrial-dark").unwrap().chamfer, [8.0, 16.0, 18.0]);
    assert_eq!(Theme::builtin("night-ops").unwrap().chamfer, [8.0, 16.0, 18.0]);
    assert_eq!(Theme::builtin("zeta-remastered").unwrap().chamfer, [4.0, 8.0, 14.0]);

    let design_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/tokens/themes.json");
    for id in ["industrial-dark", "night-ops", "zeta-remastered"] {
        assert_eq!(
            Theme::builtin(id).unwrap().chamfer,
            Theme::from_file(design_path, id).unwrap().chamfer,
            "chamfer parity mismatch for {id}"
        );
    }
}

#[test]
fn text_scale_scales_font_and_spacing() {
    let base = Theme::dark();
    assert_eq!(base.font_px(), 14.0);
    let big = Theme::for_appearance(Appearance { text_scale: 150, ..Appearance::default() });
    assert_eq!(big.text_scale, 1.5);
    assert_eq!(big.font_px(), 21.0);
    assert_eq!(big.space(10.0), 15.0);
    assert_eq!(base.clone().with_text_scale(100.0).text_scale, 4.0);
    assert_eq!(base.with_text_scale(0.0).text_scale, 0.5);
}

#[test]
fn contrast_ratio_matches_wcag_extremes() {
    let white = Color::hex(0xFFFFFF);
    let black = Color::hex(0x000000);
    assert!((contrast_ratio(white, black) - 21.0).abs() < 0.01);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-4);
}

#[test]
fn builtin_themes_pass_contrast_audit() {
    for id in ["industrial-dark", "night-ops", "zeta-remastered"] {
        assert_eq!(Theme::builtin(id).unwrap().audit_contrast(), vec![], "{id}");
    }
    // Known finding: the amber accent on the cream light panel is about 1.8:1 (needs 3:1).
    // Fixing it is a design decision; tighten this assertion to `vec![]` once the light accent is darkened.
    let light = Theme::light().audit_contrast();
    assert_eq!(light.len(), 1, "{light:?}");
    assert_eq!((light[0].foreground, light[0].background), ("accent", "panel"));
    let mut bad = Theme::dark();
    bad.palette.text = bad.palette.panel;
    assert!(bad.audit_contrast().iter().any(|i| i.foreground == "text" && i.background == "panel"));
}

const BUILTIN_JSON: &str = include_str!("../assets/themes.json");

#[test]
fn builtin_tokens_are_a_valid_contract() {
    assert_eq!(validate_tokens(BUILTIN_JSON), vec![]);
}

#[test]
fn design_tokens_copy_matches_builtin() {
    let design = include_str!("../../../design/tokens/themes.json");
    assert!(design == BUILTIN_JSON, "design/tokens/themes.json and crates/hey-style/assets/themes.json differ");
}

#[test]
fn validation_reports_every_problem() {
    let broken = BUILTIN_JSON.replacen("\"focus\":", "\"focusX\":", 1).replacen("\"width\":", "\"width\": 99, \"w0\":", 1);
    let paths: Vec<String> = validate_tokens(&broken).into_iter().map(|i| i.path).collect();
    assert!(paths.iter().any(|p| p == "focus.width"), "{paths:?}");
    assert!(validate_tokens("{").iter().any(|i| i.path == "json"));
    assert!(validate_tokens("{}").iter().any(|i| i.path == "themes"));
}

#[test]
fn exports_css_and_design_tokens() {
    let css = export_css(BUILTIN_JSON).unwrap();
    assert!(css.contains("[data-theme=\"industrial-dark\"]"));
    assert!(css.contains("--hey-color-text-muted: #"));
    assert!(css.contains("--hey-ansi-15: #"));
    let figma: serde_json::Value = serde_json::from_str(&export_figma(BUILTIN_JSON).unwrap()).unwrap();
    assert_eq!(figma["industrial-dark"]["color"]["textMuted"]["$type"], "color");
    assert_eq!(figma["industrial-dark"]["motion"]["fast"]["$type"], "duration");
}
