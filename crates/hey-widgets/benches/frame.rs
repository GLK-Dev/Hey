// Copyright (c) Vitaly Golik (mjojo)

//! Frame benchmarks: widget paint (layout + display list) and CPU rasterization at 1080p and 4K.

use criterion::{criterion_group, criterion_main, Criterion};
use hey_core::{Callback, Color, MsgSink, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd, UiRenderer, Viewport};
use hey_layout::Style;
use hey_render_cpu::CpuRenderer;
use hey_style::{Theme, Variant};
use hey_text::Fonts;
use hey_widgets::{Button, Label, Ui};
use std::hint::black_box;
use std::sync::Arc;

fn fonts() -> (Arc<Fonts>, hey_text::FontId) {
    let mut f = Fonts::new();
    let id = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    (Arc::new(f), id)
}

/// A "typical" UI: 20 rows of a label and 5 buttons.
fn typical_ui(fonts: &Arc<Fonts>, id: hey_text::FontId) -> Ui<()> {
    let mut ui: Ui<()> = Ui::new(Theme::dark(), fonts.clone(), vec![id], MsgSink::new());
    let root = ui.root();
    for r in 0..20 {
        let row = ui.add_container(root, Style::row().gap(8.0));
        Label::add(&mut ui, row, &format!("Row {r}: the quick brown fox"));
        for b in 0..5 {
            Button::add(&mut ui, row, &format!("Button {b}"), Variant::Secondary, Callback::new(|_| ()));
        }
    }
    ui
}

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
    let (fonts, id) = fonts();
    let vp = Size::new(1920.0, 1080.0);

    c.bench_function("ui_paint_typical_1080p", |b| {
        let mut ui = typical_ui(&fonts, id);
        ui.paint(vp);
        b.iter(|| black_box(ui.paint(black_box(vp))));
    });

    c.bench_function("ui_cpu_frame_typical_1080p", |b| {
        let mut ui = typical_ui(&fonts, id);
        let mut r = CpuRenderer::new(1920, 1080);
        r.fonts = Some(fonts.clone());
        b.iter(|| {
            let list = ui.paint(vp);
            r.render(&list, Viewport { width: 1920, height: 1080, scale: 1.0 });
        });
    });

    for (name, w, h) in [("cpu_raster_2000_rects_1080p", 1920u32, 1080u32), ("cpu_raster_2000_rects_4k", 3840, 2160)] {
        let list = synthetic_list(w as f32, h as f32);
        let mut r = CpuRenderer::new(w, h);
        c.bench_function(name, |b| b.iter(|| r.render(black_box(&list), Viewport { width: w, height: h, scale: 1.0 })));
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
