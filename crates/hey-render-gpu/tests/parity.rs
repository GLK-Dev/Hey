// Copyright (c) Vitaly Golik (mjojo)

//! GPU <-> CPU parity (ADR-6): the CPU renderer is the reference.
//! Tests skip (with a note) when the machine has no GPU adapter.

use hey_core::{Color, Point, Rect};
use hey_draw::{Border, DisplayList, DrawCmd, Shadow, UiRenderer, Viewport};
use hey_render_cpu::CpuRenderer;
use hey_render_gpu::GpuRenderer;

struct Diff {
    max: u8,
    bad: usize,
}

fn compare(name: &str, w: u32, h: u32, cmds: Vec<DrawCmd>, tol: u8) -> Option<Diff> {
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
    cpu.render(&list, vp);
    gpu.clear_color = bg;
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
    if let Some(d) = compare(name, w, h, cmds, tol) {
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
