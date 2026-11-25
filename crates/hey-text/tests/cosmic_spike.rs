// Copyright (c) Vitaly Golik (mjojo)

//! Spike for ADR-8: can `cosmic-text` replace `fontdue` in `hey-text`?
//! Run with `cargo test -p hey-text --test cosmic_spike -- --nocapture`.

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Wrap};
use hey_text::{measure, Fonts};

const SIZE: f32 = 16.0;

fn font_bytes() -> Vec<u8> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    std::fs::read(p).unwrap()
}

/// `FontSystem` with the embedded Onest only, plus system fonts for fallback.
fn system(with_system_fonts: bool) -> FontSystem {
    let mut fs = if with_system_fonts {
        FontSystem::new()
    } else {
        FontSystem::new_with_fonts([]) // still loads nothing from the OS? see below
    };
    fs.db_mut().load_font_data(font_bytes());
    fs
}

struct Shaped {
    width: f32,
    lines: usize,
    glyphs: usize,
    rtl_glyphs: usize,
    fonts_used: usize,
}

fn shape(fs: &mut FontSystem, text: &str, width: Option<f32>) -> Shaped {
    let mut buf = Buffer::new(fs, Metrics::new(SIZE, SIZE * 1.3));
    buf.set_wrap(if width.is_some() { Wrap::Word } else { Wrap::None });
    buf.set_size(width, None);
    buf.set_text(text, &Attrs::new().family(Family::Name("Onest")), Shaping::Advanced, None);
    buf.shape_until_scroll(fs, false);
    let (mut w, mut lines, mut glyphs, mut rtl, mut ids) = (0f32, 0, 0, 0, std::collections::HashSet::new());
    for run in buf.layout_runs() {
        lines += 1;
        w = w.max(run.line_w);
        for g in run.glyphs {
            glyphs += 1;
            ids.insert(g.font_id);
            if g.level.is_rtl() {
                rtl += 1;
            }
        }
    }
    Shaped { width: w, lines, glyphs, rtl_glyphs: rtl, fonts_used: ids.len() }
}

#[test]
fn latin_and_cyrillic_widths_are_close_to_fontdue() {
    let mut f = Fonts::new();
    let onest = f.add_bytes(&font_bytes()).unwrap();
    let mut fs = system(true);
    for text in ["Hello, world", "Привет, мир", "The quick brown fox"] {
        let a = measure(&f, text, &[onest], SIZE);
        let b = shape(&mut fs, text, None).width;
        println!("{text:>22}: fontdue {a:7.2}  cosmic {b:7.2}  delta {:+.2}", b - a);
        // Kerning/hinting differences are expected, a few percent is fine.
        assert!((a - b).abs() / a < 0.08, "{text}: {a} vs {b}");
    }
}

#[test]
fn bidi_runs_are_reversed() {
    let mut fs = system(true);
    let s = shape(&mut fs, "abc שלום def", None);
    println!("hebrew mixed: {} glyphs, {} rtl, {} fonts", s.glyphs, s.rtl_glyphs, s.fonts_used);
    // Paragraph direction is LTR, the Hebrew run must get an RTL embedding level.
    {
        assert!(s.rtl_glyphs > 0);
    }
}

#[test]
fn arabic_shaping_uses_fallback_font() {
    let mut fs = system(true);
    let s = shape(&mut fs, "مرحبا", None);
    println!("arabic: {} glyphs, {} rtl, {} fonts, w={:.1}", s.glyphs, s.rtl_glyphs, s.fonts_used, s.width);
    assert!(s.glyphs > 0);
}

#[test]
fn emoji_and_combining_marks() {
    let mut fs = system(true);
    let s = shape(&mut fs, "e\u{0301} 👨‍👩‍👧 🙂", None);
    println!("emoji/combining: {} glyphs, {} fonts", s.glyphs, s.fonts_used);
    assert!(s.glyphs > 0);
}

#[test]
fn wraps_within_width() {
    let mut fs = system(true);
    let text = "слово слово слово слово";
    let one = shape(&mut fs, text, None).width;
    let s = shape(&mut fs, text, Some(one / 2.0));
    println!("wrap: {} lines, w={:.1} (limit {:.1})", s.lines, s.width, one / 2.0);
    assert!(s.lines >= 2);
    assert!(s.width <= one / 2.0 + 0.5);
}

#[test]
fn cold_start_and_shaping_cost() {
    let t = std::time::Instant::now();
    let mut fs = FontSystem::new();
    let cold = t.elapsed();
    fs.db_mut().load_font_data(font_bytes());
    let t = std::time::Instant::now();
    for _ in 0..200 {
        let _ = shape(&mut fs, "Привет, мир — hello world", None);
    }
    println!("FontSystem::new (system fonts): {cold:?}; 200 shapes: {:?}", t.elapsed());
}
