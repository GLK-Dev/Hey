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

// An RLO override makes Latin text behave as right-to-left without needing an Arabic/Hebrew font.
const RLO: &str = "\u{202E}";
const PDF: &str = "\u{202C}";

#[test]
fn rtl_run_is_laid_out_visually_reversed() {
    let (f, onest, _) = fonts();
    let text = format!("{RLO}abc def{PDF}");
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.wrap = false;
    let l = layout(&f, &text, &st);
    let ln = &l.lines[0];
    let glyphs: Vec<_> = ln.glyphs.iter().filter(|g| g.advance > 0.0).collect();
    assert_eq!(glyphs.len(), 7);
    assert!(glyphs.iter().all(|g| g.rtl));
    // Logical order is reversed on screen: the last character is leftmost.
    assert!(glyphs.windows(2).all(|w| w[0].byte > w[1].byte && w[0].x < w[1].x));
}

#[test]
fn rtl_paragraph_keeps_ltr_words_readable() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.wrap = false;
    st.rtl = Some(true);
    let l = layout(&f, "один два", &st);
    // The neutral space between two LTR words resolves to LTR (UAX #9 N1): one LTR run in an RTL line.
    assert!(l.lines[0].rtl);
    assert!(l.lines[0].glyphs.windows(2).all(|w| w[0].byte < w[1].byte));
}

#[test]
fn ltr_text_is_untouched_by_bidi() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.wrap = false;
    let l = layout(&f, "один два", &st);
    assert!(!l.lines[0].rtl);
    assert!(l.lines[0].glyphs.windows(2).all(|w| w[0].byte < w[1].byte));
}

#[test]
fn start_and_end_alignment_follow_direction() {
    let (f, onest, _) = fonts();
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.max_width = Some(300.0);
    st.align = Align::Start;
    let ltr = layout(&f, "abc", &st);
    assert_eq!(ltr.lines[0].glyphs[0].x, 0.0);
    st.rtl = Some(true);
    let rtl = layout(&f, "abc", &st);
    let w = rtl.lines[0].width;
    assert!((rtl.lines[0].glyphs[0].x - (300.0 - w)).abs() < 0.01);
    st.align = Align::End;
    assert_eq!(layout(&f, "abc", &st).lines[0].glyphs[0].x, 0.0);
}

#[test]
fn caret_and_hit_test_roundtrip_in_rtl_run() {
    let (f, onest, _) = fonts();
    let text = format!("{RLO}abc def{PDF}");
    let mut st = TextStyle::new(vec![onest], 16.0);
    st.wrap = false;
    let l = layout(&f, &text, &st);
    let start = RLO.len();
    for b in start..start + 7 {
        let x = l.caret_x(0, b).unwrap();
        assert_eq!(l.byte_at_x(&text, 0, x - 0.01), Some(b), "byte {b}");
    }
    // Logical end of an RTL run is its visual left edge; the start is the right edge.
    assert_eq!(l.caret_x(0, start + 7), Some(0.0));
    assert!((l.caret_x(0, start).unwrap() - l.lines[0].width).abs() < 0.01);
    assert_eq!(l.caret_x(5, 0), None);
}

#[test]
fn kerning_tightens_pairs() {
    let (f, onest, _) = fonts();
    let pair = measure(&f, "AV", &[onest], 48.0);
    let apart = measure(&f, "A", &[onest], 48.0) + measure(&f, "V", &[onest], 48.0);
    assert!(pair < apart - 0.5, "pair {pair} apart {apart}");
}

#[test]
fn contextual_alternates_substitute_glyphs() {
    let (f, _, mono) = fonts();
    let l = layout(&f, "->", &TextStyle::new(vec![mono], 16.0));
    let ids: Vec<u16> = l.lines[0].glyphs.iter().map(|g| g.id).collect();
    let plain = [f.glyph_index(mono, '-').unwrap(), f.glyph_index(mono, '>').unwrap()];
    assert_ne!(ids, plain, "the `->` ligature must replace the plain glyphs");
}

#[test]
fn arabic_letters_take_joined_forms() {
    let path = "C:/Windows/Fonts/arial.ttf";
    let Ok(bytes) = std::fs::read(path) else { return }; // no Arabic-capable font on this machine
    let mut f = Fonts::new();
    let arial = f.add_bytes(&bytes).unwrap();
    let text = "بيت";
    let l = layout(&f, text, &TextStyle::new(vec![arial], 24.0));
    let g = &l.lines[0].glyphs;
    assert!(l.lines[0].rtl);
    assert!(g.iter().all(|g| g.rtl));
    let isolated: Vec<u16> = text.chars().map(|c| f.glyph_index(arial, c).unwrap()).collect();
    let shaped: Vec<u16> = g.iter().map(|g| g.id).collect();
    assert_ne!(shaped, isolated, "letters must take joined forms");
}

#[test]
fn brackets_mirror_in_rtl_text() {
    let Ok(bytes) = std::fs::read("C:/Windows/Fonts/arial.ttf") else { return };
    let mut f = Fonts::new();
    let arial = f.add_bytes(&bytes).unwrap();
    let open = f.glyph_index(arial, '(').unwrap();
    let close = f.glyph_index(arial, ')').unwrap();
    // Logical "(בית)": in an RTL paragraph the first glyph drawn on the right is the
    // mirrored "(" which looks like ")".
    let l = layout(&f, "(בית)", &TextStyle::new(vec![arial], 24.0));
    let ids: Vec<u16> = l.lines[0].glyphs.iter().map(|g| g.id).collect();
    assert_eq!(ids.len(), 5);
    // Visual order is left to right: logical last char first.
    assert_eq!(ids[0], open, "closing bracket is drawn mirrored on the left");
    assert_eq!(ids[4], close, "opening bracket is drawn mirrored on the right");
}

#[test]
fn caret_moves_inside_a_ligature() {
    let Ok(bytes) = std::fs::read("C:/Windows/Fonts/arial.ttf") else { return }; // no Arabic-capable font here
    let mut f = Fonts::new();
    let arial = f.add_bytes(&bytes).unwrap();
    let text = "لا"; // lam + alef form one mandatory ligature glyph
    let l = layout(&f, text, &TextStyle::new(vec![arial], 32.0));
    let g = &l.lines[0].glyphs;
    let lig = g.iter().find(|g| g.end - g.byte > 2 && g.advance > 0.0).expect("lam-alef must ligate");
    let mid = l.caret_x(0, lig.byte + 2).unwrap();
    assert!(mid > lig.x + 0.5 && mid < lig.x + lig.advance - 0.5, "caret {mid} must fall inside the ligature");
    assert_eq!(l.byte_at_x(text, 0, mid), Some(lig.byte + 2));
    assert_eq!(l.byte_at_x(text, 0, lig.x + 0.1), Some(lig.end));
    assert_eq!(l.byte_at_x(text, 0, lig.x + lig.advance - 0.1), Some(lig.byte));
}
