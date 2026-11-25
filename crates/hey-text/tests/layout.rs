// Copyright (c) Vitaly Golik (mjojo)

use hey_text::{layout, measure, Align, Fonts, TextStyle};

fn fonts() -> (Fonts, u32, u32) {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts");
    let mut f = Fonts::new();
    let onest = f.add_file(format!("{root}/onest/Onest[wght].ttf")).unwrap();
    let mono = f.add_file(format!("{root}/jetbrainsmono/JetBrainsMono[wght].ttf")).unwrap();
    (f, onest, mono)
}

#[test]
fn measures_and_scales() {
    let (f, onest, _) = fonts();
    let w16 = measure(&f, "Привет, мир", &[onest], 16.0);
    let w32 = measure(&f, "Привет, мир", &[onest], 32.0);
    assert!(w16 > 40.0, "{w16}");
    assert!((w32 / w16 - 2.0).abs() < 0.05);
}

#[test]
fn wraps_on_words_within_width() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    let one = measure(&f, "слово слово слово слово", &[onest], 16.0);
    st.max_width = Some(one / 2.0);
    let l = layout(&f, "слово слово слово слово", &st);
    assert!(l.lines.len() >= 2);
    assert!(l.lines.iter().all(|x| x.width <= one / 2.0 + 0.01));
    assert!(l.height > 16.0 * l.lines.len() as f32 * 0.9);
}

#[test]
fn newline_makes_lines_and_long_word_breaks() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    assert_eq!(layout(&f, "a\nb\n\nc", &st).lines.len(), 4);
    st.max_width = Some(30.0);
    let l = layout(&f, "Длинноеслововбезпробелов", &st);
    assert!(l.lines.len() > 1);
    assert!(l.lines.iter().all(|x| x.width <= 30.0 + 0.01));
}

#[test]
fn alignment_offsets_lines() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.max_width = Some(200.0);
    st.align = Align::Right;
    let l = layout(&f, "abc", &st);
    let g = l.lines[0].glyphs[0];
    assert!((g.x - (200.0 - l.lines[0].width)).abs() < 0.01);
    st.align = Align::Center;
    let g = layout(&f, "abc", &st).lines[0].glyphs[0];
    assert!((g.x - (200.0 - l.lines[0].width) / 2.0).abs() < 0.01);
}

#[test]
fn ellipsis_truncates_to_max_lines_and_width() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.max_width = Some(80.0);
    st.max_lines = Some(2);
    st.ellipsis = true;
    let l = layout(&f, "очень длинный текст который точно не поместится в две строки", &st);
    assert_eq!(l.lines.len(), 2);
    assert!(l.lines.iter().all(|x| x.width <= 80.01));
    let dots = f.glyph_index(onest, '…').unwrap();
    assert_eq!(l.lines[1].glyphs.last().unwrap().id, dots);

    st.wrap = false;
    st.max_lines = Some(1);
    let l = layout(&f, "очень длинная однострочная надпись", &st);
    assert_eq!(l.lines.len(), 1);
    assert!(l.lines[0].width <= 80.01);
}

#[test]
fn fallback_uses_next_font_for_missing_glyph() {
    let (f, onest, mono) = fonts();
    // Pick a character that is absent from Onest but present in JetBrains Mono.
    let ch = ('\u{2190}'..'\u{2600}')
        .find(|&c| f.glyph_index(onest, c).is_none() && f.glyph_index(mono, c).is_some())
        .expect("need a character present only in the mono font");
    let s = format!("a{ch}b");
    let l = layout(&f, &s, &TextStyle::new(vec![onest, mono], 16.0));
    let fonts: Vec<_> = l.lines[0].glyphs.iter().map(|g| g.font).collect();
    assert_eq!(fonts, [onest, mono, onest]);
    let runs = l.to_runs(hey_core::Point::new(0.0, 0.0), hey_core::Color::rgb(255, 255, 255));
    assert_eq!(runs.len(), 3);
}

#[test]
fn bitmap_is_cached_and_nonempty() {
    let (f, onest, _) = fonts();
    let g = f.glyph_index(onest, 'Ж').unwrap();
    let a = f.bitmap(onest, g, 24.0);
    let b = f.bitmap(onest, g, 24.0);
    assert!(std::sync::Arc::ptr_eq(&a, &b));
    assert!(a.w > 0 && a.h > 0 && a.data.iter().any(|&v| v > 200));
}
