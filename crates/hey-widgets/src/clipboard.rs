// Copyright (c) Vitaly Golik (mjojo)

//! Shared clipboard for text widgets. In-process memory by default;
//! the shell replaces it with the system one via `set_clipboard`.

use hey_text::{Clipboard, MemoryClipboard};
use std::cell::RefCell;

thread_local! {
    static CLIPBOARD: RefCell<Box<dyn Clipboard>> = RefCell::new(Box::new(MemoryClipboard::default()));
}

/// Replace the clipboard (call on the UI thread).
pub fn set_clipboard(cb: Box<dyn Clipboard>) {
    CLIPBOARD.with(|c| *c.borrow_mut() = cb);
}

/// Handle to the shared clipboard, implements `Clipboard`.
#[derive(Default, Clone, Copy)]
pub struct SharedClipboard;

impl Clipboard for SharedClipboard {
    fn get(&mut self) -> Option<String> {
        CLIPBOARD.with(|c| c.borrow_mut().get())
    }
    fn set(&mut self, text: String) {
        CLIPBOARD.with(|c| c.borrow_mut().set(text));
    }
}
