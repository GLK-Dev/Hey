// Copyright (c) Vitaly Golik (mjojo)

//! GPU <-> CPU parity (ADR-6): the CPU renderer is the reference.
//! Tests skip (with a note) when the machine has no GPU adapter.

use std::sync::Arc;

use hey_core::{Color, Point, Rect};
use hey_draw::{Border, DisplayList, DrawCmd, ImageId, ImageStore, Shadow, UiRenderer, Viewport};
use hey_render_cpu::CpuRenderer;
use hey_render_gpu::GpuRenderer;
use hey_text::{layout, Align, Fonts, TextStyle};

struct Diff {
    max: u8,
    bad: usize,
}

#[derive(Default)]
struct Assets {
    fonts: Option<Arc<Fonts>>,
    images: Option<Arc<ImageStore>>,
}

fn compare(name: &str, w: u32, h: u32, cmds: Vec<DrawCmd>, tol: u8, assets: Assets) -> Option<Diff> {
    let mut gpu = match GpuRenderer::new_headless(w, h) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{name}: skipped ({e})");
            return None;
        }
    };
    let bg = Color::hex(0x0B0C10);
    let mut list = DisplayList::new();
    cmds.into_iter().for_each(|c| list.push(c));
    let vp = Viewport { width: w, height: h, scale: 1.0 };

    let mut cpu = CpuRenderer::new(w, h);
    cpu.clear_color = bg;
    cpu.fonts = assets.fonts.clone();
    cpu.images = assets.images.clone();
    cpu.render(&list, vp);
    gpu.clear_color = bg;
    gpu.fonts = assets.fonts;
    gpu.set_images(assets.images);
    gpu.render(&list, vp);
    let g = gpu.read_pixels();

    let mut d = Diff { max: 0, bad: 0 };
    for (i, (a, b)) in cpu.fb.data.iter().zip(&g).enumerate() {
        let e = a.abs_diff(*b);
        d.max = d.max.max(e);
        if e > tol {
            if d.bad < 5 {
                eprintln!("{name}: px ({}, {}) ch {} cpu {a} gpu {b}", (i / 4) as u32 % w, (i / 4) as u32 / w, i % 4);
            }
            d.bad += 1;
        }
    }
    eprintln!("{name}: max channel diff {}, over tolerance {}: {}", d.max, tol, d.bad);
    Some(d)
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

fn check(name: &str, w: u32, h: u32, cmds: Vec<DrawCmd>, tol: u8) {
    check_with(name, w, h, cmds, tol, Assets::default());
}

fn check_with(name: &str, w: u32, h: u32, cmds: Vec<DrawCmd>, tol: u8, assets: Assets) {
    if let Some(d) = compare(name, w, h, cmds, tol, assets) {
        assert_eq!(d.bad, 0, "{name}: {} channels differ by more than {tol} (max {})", d.bad, d.max);
    }
}

// Small tolerance covers float rounding and the difference between CPU f32 and GPU unorm blending.
const TOL: u8 = 3;

#[test]
fn parity_rect() {
    check("rect", 64, 64, vec![rect(Rect::new(8.0, 8.0, 48.0, 32.0), 0x2A6FDB, None, 0.0, None)], TOL);
    check("rect_fractional", 64, 64, vec![rect(Rect::new(8.3, 8.6, 20.4, 31.7), 0x2A6FDB, None, 0.0, None)], TOL);
}

#[test]
fn parity_chamfer() {
    check(
        "chamfer",
        64,
        64,
        vec![
            rect(Rect::new(4.0, 4.0, 24.0, 24.0), 0xE0A030, None, 5.0, None),
            rect(Rect::new(32.0, 4.0, 28.0, 28.0), 0xE0A030, None, 9.0, None),
            rect(Rect::new(4.0, 34.0, 56.0, 26.0), 0xE0A030, None, 18.0, None),
        ],
        TOL,
    );
}

#[test]
fn parity_border() {
    check("border", 64, 64, vec![rect(Rect::new(8.5, 8.5, 48.0, 48.0), 0x1B2030, Some((2.0, 0x66CCFF)), 9.0, None)], TOL);
}

#[test]
fn parity_shadow() {
    let s = Shadow { offset: Point::new(0.0, 4.0), blur: 12.0, color: Color::rgba(0, 0, 0, 160) };
    check("shadow", 96, 96, vec![rect(Rect::new(24.0, 20.0, 48.0, 40.0), 0x2A6FDB, None, 5.0, Some(s))], TOL);
}

#[test]
fn parity_alpha_and_overlap() {
    check(
        "alpha",
        64,
        64,
        vec![
            DrawCmd::fill(Rect::new(8.0, 8.0, 30.0, 30.0), Color::rgba(255, 0, 0, 180)),
            DrawCmd::fill(Rect::new(24.0, 24.0, 30.0, 30.0), Color::rgba(0, 120, 255, 128)),
        ],
        TOL,
    );
}

#[test]
fn parity_clip() {
    check(
        "clip",
        64,
        64,
        vec![
            DrawCmd::ClipPush(Rect::new(8.0, 8.0, 32.0, 32.0)),
            DrawCmd::fill(Rect::new(0.0, 0.0, 64.0, 64.0), Color::hex(0x2A6FDB)),
            DrawCmd::ClipPush(Rect::new(20.0, 20.0, 40.0, 40.0)),
            DrawCmd::fill(Rect::new(0.0, 0.0, 64.0, 64.0), Color::hex(0xE0A030)),
            DrawCmd::ClipPop,
            DrawCmd::ClipPop,
        ],
        TOL,
    );
}

#[test]
fn parity_line() {
    let line = |a: (f32, f32), b: (f32, f32), w: f32| DrawCmd::Line {
        a: Point::new(a.0, a.1),
        b: Point::new(b.0, b.1),
        width: w,
        color: Color::hex(0x66CCFF),
    };
    check(
        "line",
        64,
        64,
        vec![line((4.0, 4.5), (60.0, 4.5), 1.0), line((6.0, 10.0), (50.0, 58.0), 2.0), line((30.0, 30.0), (30.0, 30.0), 6.0)],
        TOL,
    );
}

#[test]
fn parity_text() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts");
    let mut f = Fonts::new();
    let onest = f.add_file(format!("{root}/onest/Onest[wght].ttf")).unwrap();
    let mono = f.add_file(format!("{root}/jetbrainsmono/JetBrainsMono[wght].ttf")).unwrap();
    let mut st = TextStyle::new(vec![onest, mono], 18.0);
    st.max_width = Some(220.0);
    let l = layout(&f, "Привет, Hey! Текст переносится по словам и обрезается.", &st);
    let mut cmds: Vec<DrawCmd> = vec![DrawCmd::fill(Rect::new(0.0, 0.0, 120.0, 60.0), Color::hex(0x2A6FDB))];
    cmds.extend(l.to_runs(Point::new(10.0, 10.0), Color::hex(0xE6E8EE)).into_iter().map(DrawCmd::Text));
    let mut st2 = TextStyle::new(vec![mono], 14.0);
    st2.max_width = Some(220.0);
    st2.align = Align::Right;
    let l2 = layout(&f, "fn main() {}", &st2);
    cmds.extend(l2.to_runs(Point::new(10.0, 110.0), Color::hex(0x4FC3F7)).into_iter().map(DrawCmd::Text));
    let mut st3 = TextStyle::new(vec![onest], 24.0);
    st3.wrap = false;
    let l3 = layout(&f, "Обрезка клипом", &st3);
    cmds.push(DrawCmd::ClipPush(Rect::new(10.0, 140.0, 80.0, 18.0)));
    cmds.extend(l3.to_runs(Point::new(10.0, 135.0), Color::hex(0xFFB74D)).into_iter().map(DrawCmd::Text));
    cmds.push(DrawCmd::ClipPop);
    // A shape drawn after the text must cover it (painting order is preserved across pipelines).
    cmds.push(DrawCmd::fill(Rect::new(150.0, 100.0, 60.0, 30.0), Color::rgba(255, 0, 0, 200)));
    check_with("text", 240, 180, cmds, TOL, Assets { fonts: Some(Arc::new(f)), images: None });
}

#[test]
fn parity_image() {
    // 4x3 gradient with varying alpha: upscaled, downscaled, fractional rect, clipped, under a shape.
    let (iw, ih) = (4u32, 3u32);
    let mut px = Vec::new();
    for y in 0..ih {
        for x in 0..iw {
            px.extend_from_slice(&[(x * 70) as u8, (y * 100) as u8, 200 - (x * 40) as u8, 255 - ((x + y) * 30) as u8]);
        }
    }
    let mut store = ImageStore::new();
    let a = store.add(iw, ih, px).unwrap();
    let b = store.add(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 128]).unwrap();
    let cmds = vec![
        DrawCmd::fill(Rect::new(0.0, 0.0, 128.0, 96.0), Color::hex(0x2A6FDB)),
        DrawCmd::Image { rect: Rect::new(4.0, 4.0, 40.0, 30.0), image: a },
        DrawCmd::Image { rect: Rect::new(50.3, 4.6, 2.0, 3.0), image: a },
        DrawCmd::Image { rect: Rect::new(60.0, 4.0, 31.7, 21.2), image: b },
        DrawCmd::ClipPush(Rect::new(10.0, 40.0, 30.0, 20.0)),
        DrawCmd::Image { rect: Rect::new(4.0, 36.0, 60.0, 45.0), image: a },
        DrawCmd::ClipPop,
        DrawCmd::Image { rect: Rect::new(70.0, 40.0, 40.0, 40.0), image: b },
        DrawCmd::fill(Rect::new(90.0, 60.0, 30.0, 30.0), Color::rgba(255, 0, 0, 160)),
        // Unknown id and an empty (after snapping) rect are ignored by both renderers.
        DrawCmd::Image { rect: Rect::new(0.0, 0.0, 10.0, 10.0), image: ImageId(99) },
        DrawCmd::Image { rect: Rect::new(0.0, 0.0, 0.2, 10.0), image: a },
    ];
    check_with("image", 128, 96, cmds, TOL, Assets { fonts: None, images: Some(Arc::new(store)) });
}
