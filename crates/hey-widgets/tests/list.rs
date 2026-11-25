// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{List, Ui};

fn key(c: &str) -> UiEvent {
    UiEvent::KeyDown { code: c.into(), mods: Modifiers::default() }
}

#[test]
fn list_select_keys_wheel() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    let mut ui: Ui<usize> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], sink.clone());
    let root = ui.root();
    let items: Vec<String> = (0..50).map(|i| format!("Строка {i}")).collect();
    let l = List::add(&mut ui, root, items, 200.0, 5, Callback::new(|i| i));
    let vp = Size::new(400.0, 300.0);
    ui.paint(vp);
    let r = ui.tree.rect(l);

    // клик по первой строке
    let p = Point::new(r.x + 20.0, r.y + 5.0);
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(sink.drain(), [0]);

    ui.event(&key("ArrowDown"));
    ui.event(&key("ArrowDown"));
    assert_eq!(sink.drain(), [1, 2]);
    ui.event(&key("End"));
    assert_eq!(sink.drain(), [49]);
    ui.event(&key("ArrowDown"));
    assert!(sink.is_empty(), "на последней строке выбор не меняется");

    // выбранная строка прокручена в поле зрения: рисуются не все 50 строк
    let list = ui.paint(vp);
    let texts = list.cmds().iter().filter(|c| matches!(c, DrawCmd::Text(_))).count();
    assert!(texts > 0 && texts <= 7, "рисуются только видимые строки: {texts}");

    ui.event(&UiEvent::Wheel { pos: p, dx: 0.0, dy: 100000.0 });
    ui.event(&key("Home"));
    assert_eq!(sink.drain(), [0]);
    ui.paint(vp);
}
