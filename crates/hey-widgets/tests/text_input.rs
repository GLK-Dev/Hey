// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{TextInput, Ui};

fn key(c: &str, shift: bool, ctrl: bool) -> UiEvent {
    UiEvent::KeyDown { code: c.into(), mods: Modifiers { shift, ctrl, ..Default::default() } }
}

#[test]
fn typing_selection_clipboard_undo() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    let mut ui: Ui<String> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], sink.clone());
    let root = ui.root();
    let input = TextInput::add(&mut ui, root, "", 200.0, Callback::new(|s| s));
    let vp = Size::new(400.0, 100.0);
    ui.paint(vp);

    assert!(!ui.event(&UiEvent::Text("x".into())), "без фокуса текст игнорируется");
    ui.event(&key("Tab", false, false));
    assert_eq!(ui.focused(), Some(input));
    ui.event(&UiEvent::Text("Привет".into()));
    ui.event(&key("ArrowLeft", true, false));
    ui.event(&key("c", false, true));
    ui.event(&key("End", false, false));
    ui.event(&key("v", false, true));
    assert_eq!(sink.drain().last().unwrap(), "Приветт");
    ui.event(&key("Backspace", false, false));
    ui.event(&key("z", false, true));
    assert_eq!(sink.drain().last().unwrap(), "Приветт");
    ui.event(&key("a", false, true));
    ui.event(&UiEvent::Text("Я".into()));
    assert_eq!(sink.drain(), ["Я"]);
    ui.event(&key("ArrowLeft", false, false));
    assert!(sink.is_empty(), "движение каретки не шлёт сообщений");

    ui.paint(vp);
    let r = ui.tree.rect(input);
    let p = Point::new(r.x + 1.0, r.y + r.h / 2.0);
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::Text("_".into()));
    assert_eq!(sink.drain(), ["_Я"]);
    ui.paint(vp);
}
