// Copyright (c) Vitaly Golik (mjojo)

//! Property tests: random edit sequences must keep `TextEditState` invariants and undo/redo must be lossless.

use hey_text::{MemoryClipboard, TextEditState};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    Insert(String),
    Backspace,
    Delete,
    DeleteWordLeft,
    DeleteWordRight,
    Left(bool),
    Right(bool),
    WordLeft(bool),
    WordRight(bool),
    Home(bool),
    End(bool),
    SelectAll,
    SelectWordAt(usize),
    SetCaret(usize, bool),
    Cut,
    Paste,
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        "[a-zA-Z0-9 \u{e9}\u{416}\u{4e2d}\u{1f600}\n]{0,6}".prop_map(Op::Insert),
        Just(Op::Backspace),
        Just(Op::Delete),
        Just(Op::DeleteWordLeft),
        Just(Op::DeleteWordRight),
        any::<bool>().prop_map(Op::Left),
        any::<bool>().prop_map(Op::Right),
        any::<bool>().prop_map(Op::WordLeft),
        any::<bool>().prop_map(Op::WordRight),
        any::<bool>().prop_map(Op::Home),
        any::<bool>().prop_map(Op::End),
        Just(Op::SelectAll),
        (0usize..64).prop_map(Op::SelectWordAt),
        (0usize..64, any::<bool>()).prop_map(|(p, e)| Op::SetCaret(p, e)),
        Just(Op::Cut),
        Just(Op::Paste),
        Just(Op::Undo),
        Just(Op::Redo),
    ]
}

fn apply(st: &mut TextEditState, cb: &mut MemoryClipboard, op: &Op) {
    match op {
        Op::Insert(s) => st.insert(s),
        Op::Backspace => st.backspace(),
        Op::Delete => st.delete(),
        Op::DeleteWordLeft => st.delete_word_left(),
        Op::DeleteWordRight => st.delete_word_right(),
        Op::Left(e) => st.move_left(*e),
        Op::Right(e) => st.move_right(*e),
        Op::WordLeft(e) => st.move_word_left(*e),
        Op::WordRight(e) => st.move_word_right(*e),
        Op::Home(e) => st.home(*e),
        Op::End(e) => st.end(*e),
        Op::SelectAll => st.select_all(),
        Op::SelectWordAt(p) => st.select_word_at(*p),
        Op::SetCaret(p, e) => st.set_caret(*p, *e),
        Op::Cut => st.cut(cb),
        Op::Paste => st.paste(cb),
        Op::Undo => {
            st.undo();
        }
        Op::Redo => {
            st.redo();
        }
    }
}

proptest! {
    #[test]
    fn invariants_hold(initial in "[a-z \u{416}\u{1f600}]{0,10}", ops in prop::collection::vec(op(), 0..60)) {
        let mut st = TextEditState::new(initial);
        let mut cb = MemoryClipboard::default();
        for o in &ops {
            apply(&mut st, &mut cb, o);
            let t = st.text();
            prop_assert!(st.caret() <= t.len() && st.anchor() <= t.len());
            prop_assert!(t.is_char_boundary(st.caret()) && t.is_char_boundary(st.anchor()));
            let r = st.selection();
            prop_assert!(r.start <= r.end && r.end <= t.len());
        }
    }

    #[test]
    fn undo_all_restores_initial_and_redo_all_restores_final(
        initial in "[a-z \u{416}]{0,10}",
        ops in prop::collection::vec(op(), 0..40),
    ) {
        let mut st = TextEditState::new(initial.clone());
        let mut cb = MemoryClipboard::default();
        for o in ops.iter().filter(|o| !matches!(o, Op::Undo | Op::Redo)) {
            apply(&mut st, &mut cb, o);
        }
        let final_text = st.text().to_string();
        while st.undo() {}
        prop_assert_eq!(st.text(), initial.as_str());
        while st.redo() {}
        prop_assert_eq!(st.text(), final_text.as_str());
    }
}
