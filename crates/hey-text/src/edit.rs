// Copyright (c) Vitaly Golik (mjojo)

//! Pure text editing model.
//!
//! Positions are byte offsets into UTF-8, always on grapheme boundaries.
//! Selection is between `anchor` and `caret`; `caret == anchor` means no selection.

use unicode_segmentation::UnicodeSegmentation;

pub trait Clipboard {
    fn get(&mut self) -> Option<String>;
    fn set(&mut self, text: String);
}

#[derive(Default)]
pub struct MemoryClipboard(pub Option<String>);

impl Clipboard for MemoryClipboard {
    fn get(&mut self) -> Option<String> {
        self.0.clone()
    }
    fn set(&mut self, text: String) {
        self.0 = Some(text);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditKind {
    Insert,
    Backspace,
    Delete,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: usize,
}

#[derive(Clone, Debug)]
pub struct TextEditState {
    text: String,
    caret: usize,
    anchor: usize,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last: Option<EditKind>,
}

impl Default for TextEditState {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextEditState {
    /// Caret at the end of the text.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self { text, caret: end, anchor: end, undo: vec![], redo: vec![], last: None }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    pub fn anchor(&self) -> usize {
        self.anchor
    }
    /// Selection as `start..end` (start <= end).
    pub fn selection(&self) -> std::ops::Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }
    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ---------- navigation ----------

    /// Places the caret (snapped to a grapheme boundary not after `pos`).
    pub fn set_caret(&mut self, pos: usize, extend: bool) {
        self.caret = self.snap(pos);
        if !extend {
            self.anchor = self.caret;
        }
        self.last = None;
    }

    pub fn move_left(&mut self, extend: bool) {
        let to = if self.has_selection() && !extend { self.selection().start } else { self.prev_grapheme(self.caret) };
        self.set_caret(to, extend);
    }

    pub fn move_right(&mut self, extend: bool) {
        let to = if self.has_selection() && !extend { self.selection().end } else { self.next_grapheme(self.caret) };
        self.set_caret(to, extend);
    }

    pub fn move_word_left(&mut self, extend: bool) {
        let to = self.prev_word(self.caret);
        self.set_caret(to, extend);
    }

    pub fn move_word_right(&mut self, extend: bool) {
        let to = self.next_word(self.caret);
        self.set_caret(to, extend);
    }

    pub fn home(&mut self, extend: bool) {
        self.set_caret(0, extend);
    }

    pub fn end(&mut self, extend: bool) {
        self.set_caret(self.text.len(), extend);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
        self.last = None;
    }

    /// Selects the word under `pos` (double click).
    pub fn select_word_at(&mut self, pos: usize) {
        let pos = self.snap(pos);
        for (start, w) in self.text.split_word_bound_indices() {
            let end = start + w.len();
            if pos >= start && pos <= end && !w.trim().is_empty() {
                self.anchor = start;
                self.caret = end;
                self.last = None;
                return;
            }
        }
        self.set_caret(pos, false);
    }

    // ---------- editing ----------

    pub fn insert(&mut self, s: &str) {
        if s.is_empty() && !self.has_selection() {
            return;
        }
        self.checkpoint(EditKind::Insert);
        self.replace_selection(s);
    }

    pub fn backspace(&mut self) {
        if !self.has_selection() {
            if self.caret == 0 {
                return;
            }
            self.checkpoint(EditKind::Backspace);
            self.anchor = self.prev_grapheme(self.caret);
        } else {
            self.checkpoint(EditKind::Other);
        }
        self.replace_selection("");
    }

    pub fn delete(&mut self) {
        if !self.has_selection() {
            if self.caret == self.text.len() {
                return;
            }
            self.checkpoint(EditKind::Delete);
            self.anchor = self.next_grapheme(self.caret);
        } else {
            self.checkpoint(EditKind::Other);
        }
        self.replace_selection("");
    }

    pub fn delete_word_left(&mut self) {
        if !self.has_selection() {
            if self.caret == 0 {
                return;
            }
            self.anchor = self.prev_word(self.caret);
        }
        self.checkpoint(EditKind::Other);
        self.replace_selection("");
    }

    pub fn delete_word_right(&mut self) {
        if !self.has_selection() {
            if self.caret == self.text.len() {
                return;
            }
            self.anchor = self.next_word(self.caret);
        }
        self.checkpoint(EditKind::Other);
        self.replace_selection("");
    }

    pub fn copy(&self, cb: &mut dyn Clipboard) {
        if self.has_selection() {
            cb.set(self.selected_text().to_owned());
        }
    }

    pub fn cut(&mut self, cb: &mut dyn Clipboard) {
        if self.has_selection() {
            self.copy(cb);
            self.checkpoint(EditKind::Other);
            self.replace_selection("");
        }
    }

    pub fn paste(&mut self, cb: &mut dyn Clipboard) {
        if let Some(s) = cb.get() {
            self.checkpoint(EditKind::Other);
            self.replace_selection(&s);
        }
    }

    // ---------- undo / redo ----------

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(self.snapshot());
        self.restore(prev);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(self.snapshot());
        self.restore(next);
        true
    }

    // ---------- internals ----------

    fn snapshot(&self) -> Snapshot {
        Snapshot { text: self.text.clone(), caret: self.caret, anchor: self.anchor }
    }

    fn restore(&mut self, s: Snapshot) {
        self.text = s.text;
        self.caret = s.caret;
        self.anchor = s.anchor;
        self.last = None;
    }

    /// Saves an undo snapshot; consecutive edits of the same kind without a selection are merged.
    fn checkpoint(&mut self, kind: EditKind) {
        let coalesce = kind != EditKind::Other && self.last == Some(kind) && !self.has_selection() && !self.undo.is_empty();
        if !coalesce {
            self.undo.push(self.snapshot());
        }
        self.redo.clear();
        self.last = Some(kind);
    }

    fn replace_selection(&mut self, s: &str) {
        let r = self.selection();
        self.text.replace_range(r.clone(), s);
        self.caret = r.start + s.len();
        self.anchor = self.caret;
        // A space/newline ends a "word" — the next input starts a new undo step.
        if s.contains(char::is_whitespace) {
            self.last = None;
        }
    }

    /// Nearest grapheme boundary not after `pos`.
    fn snap(&self, pos: usize) -> usize {
        if pos >= self.text.len() {
            return self.text.len();
        }
        let mut best = 0;
        for (i, _) in self.text.grapheme_indices(true) {
            if i > pos {
                break;
            }
            best = i;
        }
        best
    }

    fn prev_grapheme(&self, pos: usize) -> usize {
        self.text[..pos].grapheme_indices(true).next_back().map_or(0, |(i, _)| i)
    }

    fn next_grapheme(&self, pos: usize) -> usize {
        self.text[pos..].graphemes(true).next().map_or(pos, |g| pos + g.len())
    }

    fn prev_word(&self, pos: usize) -> usize {
        let mut to = 0;
        for (i, w) in self.text.split_word_bound_indices() {
            if i >= pos {
                break;
            }
            if w.chars().any(char::is_alphanumeric) {
                to = i;
            }
        }
        to
    }

    fn next_word(&self, pos: usize) -> usize {
        for (i, w) in self.text.split_word_bound_indices() {
            let end = i + w.len();
            if end > pos && w.chars().any(char::is_alphanumeric) {
                return end;
            }
        }
        self.text.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_backspace() {
        let mut e = TextEditState::default();
        e.insert("при");
        e.insert("вет");
        assert_eq!(e.text(), "привет");
        e.backspace();
        assert_eq!(e.text(), "приве");
        e.move_left(false);
        e.delete();
        assert_eq!(e.text(), "прив");
    }

    #[test]
    fn graphemes_are_atomic() {
        let mut e = TextEditState::new("e\u{301}🇺🇦👨‍👩‍👧x");
        e.backspace();
        e.backspace();
        assert_eq!(e.text(), "e\u{301}🇺🇦");
        e.backspace();
        assert_eq!(e.text(), "e\u{301}");
        e.backspace();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn selection_replace_and_arrows() {
        let mut e = TextEditState::new("hello world");
        e.home(false);
        e.move_right(true);
        e.move_right(true);
        assert_eq!(e.selected_text(), "he");
        e.move_right(false);
        assert_eq!((e.caret(), e.has_selection()), (2, false));
        e.select_all();
        e.insert("x");
        assert_eq!(e.text(), "x");
    }

    #[test]
    fn word_navigation_cyrillic() {
        let mut e = TextEditState::new("один, два три");
        e.home(false);
        e.move_word_right(false);
        assert_eq!(&e.text()[..e.caret()], "один");
        e.move_word_right(false);
        assert_eq!(&e.text()[..e.caret()], "один, два");
        e.move_word_left(false);
        assert_eq!(&e.text()[..e.caret()], "один, ");
        e.delete_word_left();
        assert_eq!(e.text(), "два три");
    }

    #[test]
    fn double_click_selects_word() {
        let mut e = TextEditState::new("foo bar");
        e.select_word_at(5);
        assert_eq!(e.selected_text(), "bar");
    }

    #[test]
    fn undo_coalesces_typing() {
        let mut e = TextEditState::default();
        for c in "abc".chars() {
            e.insert(&c.to_string());
        }
        e.insert(" ");
        e.insert("d");
        assert_eq!(e.text(), "abc d");
        e.undo();
        assert_eq!(e.text(), "abc ");
        e.undo();
        assert_eq!(e.text(), "");
        assert!(!e.undo());
        e.redo();
        assert_eq!(e.text(), "abc ");
        e.insert("z");
        assert!(!e.can_redo());
    }

    #[test]
    fn clipboard_roundtrip() {
        let mut cb = MemoryClipboard::default();
        let mut e = TextEditState::new("abcdef");
        e.set_caret(1, false);
        e.set_caret(4, true);
        e.cut(&mut cb);
        assert_eq!((e.text(), cb.0.as_deref()), ("aef", Some("bcd")));
        e.paste(&mut cb);
        e.paste(&mut cb);
        assert_eq!(e.text(), "abcdbcdef");
        e.undo();
        e.undo();
        e.undo();
        assert_eq!(e.text(), "abcdef");
    }

    /// Deterministic fuzzing: invariants under random operations over Unicode.
    #[test]
    fn fuzz_invariants() {
        const ALPHA: &[&str] = &["a", "Ы", " ", "\n", "e\u{301}", "🇺🇦", "👨‍👩‍👧", "😀", "-", "世"];
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut rnd = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let mut cb = MemoryClipboard::default();
        let mut e = TextEditState::default();
        for _ in 0..20_000 {
            match rnd(18) {
                0..=2 => e.insert(ALPHA[rnd(ALPHA.len())]),
                3 => e.backspace(),
                4 => e.delete(),
                5 => e.move_left(rnd(2) == 0),
                6 => e.move_right(rnd(2) == 0),
                7 => e.move_word_left(rnd(2) == 0),
                8 => e.move_word_right(rnd(2) == 0),
                9 => e.home(rnd(2) == 0),
                10 => e.end(rnd(2) == 0),
                11 => e.select_all(),
                12 => e.cut(&mut cb),
                13 => e.paste(&mut cb),
                14 => e.copy(&mut cb),
                15 => {
                    e.undo();
                }
                16 => {
                    e.redo();
                }
                _ => {
                    let n = e.text().len() + 3;
                    e.set_caret(rnd(n), rnd(2) == 0)
                }
            }
            let t = e.text();
            for p in [e.caret(), e.anchor()] {
                assert!(p <= t.len() && t.is_char_boundary(p));
                assert!(p == t.len() || t.grapheme_indices(true).any(|(i, _)| i == p), "position {p} is inside a grapheme in {t:?}");
            }
        }
    }
}
