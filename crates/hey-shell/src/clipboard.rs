// Copyright (c) Vitaly Golik (mjojo)

//! System clipboard on `arboard` for text widgets.

use hey_text::Clipboard;

/// Wrapper over `arboard::Clipboard`; silently does nothing if the clipboard is unavailable.
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

/// Install the system clipboard as the shared one for widgets (call on the UI thread before creating the window).
pub fn install_system_clipboard() {
    hey_widgets::set_clipboard(Box::new(SystemClipboard::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a desktop session (X11 or Wayland); run with `cargo test -p hey-shell -- --ignored`.
    #[test]
    #[ignore = "needs a display server"]
    fn system_clipboard_roundtrip() {
        let mut c = SystemClipboard::new();
        assert!(c.0.is_some(), "clipboard backend unavailable");
        c.set("hey clipboard test".to_string());
        assert_eq!(c.get().as_deref(), Some("hey clipboard test"));
    }
}
