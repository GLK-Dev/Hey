// Copyright (c) Vitaly Golik (mjojo)

//! Fuzzes `TextEditState`: no operation sequence may panic or leave the caret or anchor off a char boundary.

#![no_main]

use hey_text::{MemoryClipboard, TextEditState};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let split = data.len().min(16);
    let (head, ops) = data.split_at(split);
    let mut st = TextEditState::new(String::from_utf8_lossy(head).into_owned());
    let mut cb = MemoryClipboard::default();
    let mut it = ops.iter().copied();
    while let Some(code) = it.next() {
        let arg = it.next().unwrap_or(0);
        let extend = arg & 1 == 1;
        match code % 20 {
            0 => st.insert(["a", " ", "\n", "\u{416}", "\u{1f600}", "e\u{301}", "ab c"][arg as usize % 7]),
            1 => st.backspace(),
            2 => st.delete(),
            3 => st.delete_word_left(),
            4 => st.delete_word_right(),
            5 => st.move_left(extend),
            6 => st.move_right(extend),
            7 => st.move_word_left(extend),
            8 => st.move_word_right(extend),
            9 => st.home(extend),
            10 => st.end(extend),
            11 => st.select_all(),
            12 => st.select_word_at(arg as usize),
            13 => st.set_caret(arg as usize, extend),
            14 => st.cut(&mut cb),
            15 => st.paste(&mut cb),
            16 => st.copy(&mut cb),
            17 => {
                st.undo();
            }
            _ => {
                st.redo();
            }
        }
        let t = st.text();
        assert!(t.is_char_boundary(st.caret()) && t.is_char_boundary(st.anchor()));
        let r = st.selection();
        assert!(r.start <= r.end && r.end <= t.len());
    }
});
