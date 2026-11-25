// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, CursorIcon, MsgSink, Point, Size, UiEvent};
use hey_style::Theme;
use hey_text::{Clipboard, Fonts};
use hey_widgets::{set_clipboard, Checkbox, SharedClipboard, Ui};
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
enum M {
    Check(bool),
    Dropped(PathBuf),
}

fn setup() -> (Ui<M>, MsgSink<M>) {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    (Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], sink.clone()), sink)
}

#[test]
fn cursor_follows_hover() {
    let (mut ui, _s) = setup();
    let root = ui.root();
    let cb = Checkbox::add(&mut ui, root, "Флажок", false, Callback::new(M::Check));
    ui.paint(Size::new(300.0, 200.0));
    assert_eq!(ui.cursor(), CursorIcon::Default);
    let r = ui.tree.rect(cb);
    ui.event(&UiEvent::PointerMove { pos: Point::new(r.x + 3.0, r.y + 3.0) });
    assert_eq!(ui.cursor(), CursorIcon::Pointer);
    ui.event(&UiEvent::PointerLeave);
    assert_eq!(ui.cursor(), CursorIcon::Default);
}

#[test]
fn file_drop_emits_message() {
    let (mut ui, sink) = setup();
    ui.on_file_drop(Callback::new(M::Dropped));
    ui.event(&UiEvent::FileHovered("a.txt".into()));
    assert!(ui.file_hovering());
    ui.event(&UiEvent::FileDropped("a.txt".into()));
    assert!(!ui.file_hovering());
    assert_eq!(sink.drain(), [M::Dropped("a.txt".into())]);
}

#[test]
fn shared_clipboard_roundtrip() {
    let mut c = SharedClipboard;
    c.set("привет".into());
    assert_eq!(c.get().as_deref(), Some("привет"));
    set_clipboard(Box::new(hey_text::MemoryClipboard::default()));
    assert_eq!(SharedClipboard.get(), None);
}
