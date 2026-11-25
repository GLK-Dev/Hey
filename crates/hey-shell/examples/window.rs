// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Color, Point, Rect};
use hey_draw::{Border, DisplayList, DrawCmd, Shadow, Viewport};
use hey_shell::{run, ShellApp, WindowConfig};
use hey_text::{layout, Fonts, TextStyle};
use std::sync::Arc;

struct Demo {
    fonts: Arc<Fonts>,
    onest: u32,
    mono: u32,
}

impl Demo {
    fn new() -> Self {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts");
        let mut f = Fonts::new();
        let onest = f.add_file(format!("{root}/onest/Onest[wght].ttf")).expect("Onest");
        let mono = f.add_file(format!("{root}/jetbrainsmono/JetBrainsMono[wght].ttf")).expect("JetBrains Mono");
        Self { fonts: Arc::new(f), onest, mono }
    }

    fn text(&self, list: &mut DisplayList, s: &str, chain: &[u32], size: f32, width: f32, at: Point, color: Color) {
        let mut st = TextStyle::new(chain.to_vec(), size);
        st.max_width = Some(width);
        for run in layout(&self.fonts, s, &st).to_runs(at, color) {
            list.push(DrawCmd::Text(run));
        }
    }
}

impl ShellApp for Demo {
    fn fonts(&self) -> Option<Arc<Fonts>> {
        Some(self.fonts.clone())
    }

    fn draw(&mut self, vp: Viewport, list: &mut DisplayList) {
        let s = vp.scale as f32;
        let r = Rect::new(60.0 * s, 60.0 * s, 360.0 * s, 200.0 * s);
        list.push(DrawCmd::Rect {
            rect: r,
            fill: Color::hex(0x1B2030),
            border: Some(Border { width: 2.0 * s, color: Color::hex(0x66CCFF) }),
            chamfer: 18.0 * s,
            shadow: Some(Shadow { offset: Point::new(0.0, 6.0 * s), blur: 24.0 * s, color: Color::rgba(0, 0, 0, 180) }),
        });
        let pad = 24.0 * s;
        let w = r.w - 2.0 * pad;
        self.text(list, "Hey", &[self.onest], 28.0 * s, w, Point::new(r.x + pad, r.y + pad), Color::hex(0xE6E8EE));
        self.text(
            list,
            "Текст на fontdue: перенос по словам, fallback-цепочка шрифтов и обрезка по ширине.",
            &[self.onest, self.mono],
            16.0 * s,
            w,
            Point::new(r.x + pad, r.y + pad + 44.0 * s),
            Color::hex(0xB4BAC8),
        );
        self.text(list, "cargo run -p hey-shell --example window", &[self.mono], 13.0 * s, w, Point::new(r.x + pad, r.y + r.h - pad - 16.0 * s), Color::hex(0x66CCFF));
    }
}

fn main() {
    run(WindowConfig::default(), Demo::new()).unwrap();
}
