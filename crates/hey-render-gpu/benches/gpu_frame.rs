// Copyright (c) Vitaly Golik (mjojo)

//! GPU frame benchmark: 2000 bordered rects at 1080p and 4K, submit plus readback.
//! The readback is the only public GPU sync point, so the numbers include a copy to the CPU.
//! Skips with a note when the machine has no GPU adapter.

use criterion::{criterion_group, criterion_main, Criterion};
use hey_core::{Color, Rect};
use hey_draw::{Border, DisplayList, DrawCmd, UiRenderer, Viewport};
use hey_render_gpu::GpuRenderer;
use std::hint::black_box;

fn synthetic_list(w: f32, h: f32) -> DisplayList {
    let mut l = DisplayList::new();
    l.push(DrawCmd::fill(Rect::new(0.0, 0.0, w, h), Color::hex(0x14161C)));
    for i in 0..2000 {
        let x = (i % 50) as f32 * (w / 50.0);
        let y = (i / 50) as f32 * (h / 40.0);
        l.push(DrawCmd::Rect {
            rect: Rect::new(x + 2.0, y + 2.0, w / 50.0 - 4.0, h / 40.0 - 4.0),
            fill: Color::hex(0x2A2E38),
            border: Some(Border { width: 1.0, color: Color::hex(0x5A6070) }),
            chamfer: 3.0,
            shadow: None,
        });
    }
    l
}

fn bench(c: &mut Criterion) {
    for (name, w, h) in [("gpu_2000_rects_1080p", 1920u32, 1080u32), ("gpu_2000_rects_4k", 3840, 2160)] {
        let mut r = match GpuRenderer::new_headless(w, h) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{name}: skipped ({e})");
                continue;
            }
        };
        let list = synthetic_list(w as f32, h as f32);
        let vp = Viewport { width: w, height: h, scale: 1.0 };
        c.bench_function(name, |b| {
            b.iter(|| {
                r.render(black_box(&list), vp);
                black_box(r.read_pixels());
            })
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
