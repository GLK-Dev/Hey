// Copyright (c) Vitaly Golik (mjojo)

//! Общий буфер обмена для текстовых виджетов. По умолчанию — в памяти процесса;
//! оболочка подменяет его системным через `set_clipboard`.

use hey_text::{Clipboard, MemoryClipboard};
use std::cell::RefCell;

thread_local! {
    static CLIPBOARD: RefCell<Box<dyn Clipboard>> = RefCell::new(Box::new(MemoryClipboard::default()));
}

/// Заменить буфер обмена (вызывать в потоке UI).
pub fn set_clipboard(cb: Box<dyn Clipboard>) {
    CLIPBOARD.with(|c| *c.borrow_mut() = cb);
}

/// Дескриптор общего буфера, реализует `Clipboard`.
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
