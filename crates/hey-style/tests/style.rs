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
