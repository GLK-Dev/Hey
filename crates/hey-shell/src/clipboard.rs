// Copyright (c) Vitaly Golik (mjojo)

//! Системный буфер обмена на `arboard` для текстовых виджетов.

use hey_text::Clipboard;

/// Обёртка над `arboard::Clipboard`; при недоступности буфера молча ничего не делает.
pub struct SystemClipboard(Option<arboard::Clipboard>);

impl SystemClipboard {
    pub fn new() -> Self {
        SystemClipboard(arboard::Clipboard::new().ok())
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard for SystemClipboard {
    fn get(&mut self) -> Option<String> {
        self.0.as_mut()?.get_text().ok()
    }
    fn set(&mut self, text: String) {
        if let Some(c) = self.0.as_mut() {
            let _ = c.set_text(text);
        }
    }
}

/// Установить системный буфер как общий для виджетов (вызывать в UI-потоке до создания окна).
pub fn install_system_clipboard() {
    hey_widgets::set_clipboard(Box::new(SystemClipboard::new()));
}
