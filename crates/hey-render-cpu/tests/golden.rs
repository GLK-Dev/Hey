// Copyright (c) Vitaly Golik (mjojo)

//! Golden-тесты: кадр сравнивается с `tests/golden/<имя>.png`.
//! Нет эталона или `HEY_UPDATE_GOLDEN=1` — эталон (пере)записывается.

use hey_core::{Color, Point, Rect};
use hey_draw::{Border, DisplayList, DrawCmd, Shadow, UiRenderer, Viewport};
use hey_render_cpu::{CpuRenderer, Framebuffer};
use std::path::PathBuf;

fn check(name: &str, w: u32, h: u32, cmds: Vec<DrawCmd>) {
    check_with(name, w, h, cmds, None);
}

fn check_with(name: &str, w: u32, h: u32, cmds: Vec<DrawCmd>, fonts: Option<std::sync::Arc<hey_text::Fonts>>) {
    let mut r = CpuRenderer::new(w, h);
    r.fonts = fonts;
    r.clear_color = Color::hex(0x0B0C10);
    let mut l = DisplayList::new();
    cmds.into_iter().for_each(|c| l.push(c));
    r.render(&l, Viewport { width: w, height: h, scale: 1.0 });

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.png"));
    if std::env::var_os("HEY_UPDATE_GOLDEN").is_some() || !path.exists() {
        r.fb.save_png(&path).unwrap();
        return;
    }
    let want = Framebuffer::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!((want.width, want.height), (w, h), "{name}: размер");
    assert!(want.data == r.fb.data, "{name}: кадр отличается от эталона (HEY_UPDATE_GOLDEN=1 для обновления)");
}

fn rect(r: Rect, fill: u32, border: Option<(f32, u32)>, chamfer: f32, shadow: Option<Shadow>) -> DrawCmd {
    DrawCmd::Rect {
        rect: r,
        fill: Color::hex(fill),
        border: border.map(|(width, c)| Border { width, color: Color::hex(c) }),
        chamfer,
        shadow,
    }
}

#[test]
fn golden_rect() {
    check("rect", 64, 64, vec![rect(Rect::new(8.0, 8.0, 48.0, 32.0), 0x2A6FDB, None, 0.0, None)]);
}

#[test]
fn golden_chamfer() {
    check(
        "chamfer",
        64,
        64,
        vec![
            rect(Rect::new(4.0, 4.0, 24.0, 24.0), 0xE0A030, None, 5.0, None),
            rect(Rect::new(32.0, 4.0, 28.0, 28.0), 0xE0A030, None, 9.0, None),
            rect(Rect::new(4.0, 34.0, 56.0, 26.0), 0xE0A030, None, 18.0, None),
        ],
    );
}

#[test]
fn golden_border() {
    check("border", 64, 64, vec![rect(Rect::new(8.5, 8.5, 48.0, 48.0), 0x1B2030, Some((2.0, 0x66CCFF)), 9.0, None)]);
}

#[test]
fn golden_shadow() {
    let s = Shadow { offset: Point::new(0.0, 4.0), blur: 12.0, color: Color::rgba(0, 0, 0, 160) };
    check("shadow", 96, 96, vec![rect(Rect::new(24.0, 20.0, 48.0, 40.0), 0x2A6FDB, None, 5.0, Some(s))]);
}

#[test]
fn golden_clip() {
    check(
        "clip",
        64,
        64,
        vec![
            DrawCmd::ClipPush(Rect::new(8.0, 8.0, 32.0, 32.0)),
            rect(Rect::new(0.0, 0.0, 64.0, 64.0), 0xCC3355, None, 18.0, None),
            DrawCmd::Line { a: Point::new(0.0, 0.0), b: Point::new(64.0, 64.0), width: 3.0, color: Color::hex(0xFFFFFF) },
            DrawCmd::ClipPop,
        ],
    );
}

#[test]
fn golden_text() {
    use hey_text::{layout, Align, Fonts, TextStyle};
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts");
    let mut f = Fonts::new();
    let onest = f.add_file(format!("{root}/onest/Onest[wght].ttf")).unwrap();
    let mono = f.add_file(format!("{root}/jetbrainsmono/JetBrainsMono[wght].ttf")).unwrap();
    let mut st = TextStyle::new(vec![onest, mono], 18.0);
    st.max_width = Some(220.0);
    let l = layout(&f, "Привет, Hey! Текст переносится по словам и обрезается.", &st);
    let mut cmds: Vec<DrawCmd> = l.to_runs(Point::new(10.0, 10.0), Color::hex(0xE6E8EE)).into_iter().map(DrawCmd::Text).collect();
    let mut st2 = TextStyle::new(vec![mono], 14.0);
    st2.max_width = Some(220.0);
    st2.align = Align::Right;
    let l2 = layout(&f, "fn main() {}", &st2);
    cmds.extend(l2.to_runs(Point::new(10.0, 110.0), Color::hex(0x4FC3F7)).into_iter().map(DrawCmd::Text));
    // Клип: текст обрезается по прямоугольнику.
    let mut st3 = TextStyle::new(vec![onest], 24.0);
    st3.wrap = false;
    let l3 = layout(&f, "Обрезка клипом", &st3);
    cmds.push(DrawCmd::ClipPush(Rect::new(10.0, 140.0, 80.0, 18.0)));
    cmds.extend(l3.to_runs(Point::new(10.0, 135.0), Color::hex(0xFFB74D)).into_iter().map(DrawCmd::Text));
    cmds.push(DrawCmd::ClipPop);
    check_with("text", 240, 180, cmds, Some(std::sync::Arc::new(f)));
}
