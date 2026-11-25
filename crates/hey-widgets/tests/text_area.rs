// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{TextArea, TextInput, Ui};
use std::rc::Rc;

fn key(c: &str, shift: bool, ctrl: bool) -> UiEvent {
    UiEvent::KeyDown { code: c.into(), mods: Modifiers { shift, ctrl, ..Default::default() } }
}

fn ui() -> (Ui<String>, MsgSink<String>) {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    (Ui::new(Theme::dark(), Rc::new(f), vec![onest], sink.clone()), sink)
}

fn down(ui: &mut Ui<String>, p: Point) {
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
}
fn up(ui: &mut Ui<String>, p: Point) {
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
}

#[test]
fn area_enter_arrows_and_wheel() {
    let (mut ui, sink) = ui();
    let root = ui.root();
    let a = TextArea::add(&mut ui, root, "", 200.0, 3, Callback::new(|s| s));
    let vp = Size::new(400.0, 300.0);
    ui.paint(vp);
    ui.event(&key("Tab", false, false));
    assert_eq!(ui.focused(), Some(a));
    ui.event(&UiEvent::Text("ab".into()));
    ui.event(&key("Enter", false, false));
    ui.event(&UiEvent::Text("cd".into()));
    assert_eq!(sink.drain().last().unwrap(), "ab\ncd");
    ui.event(&key("ArrowUp", false, false));
    ui.event(&UiEvent::Text("X".into()));
    assert_eq!(sink.drain().last().unwrap(), "abX\ncd");
    ui.event(&key("ArrowDown", false, false));
    ui.event(&UiEvent::Text("Y".into()));
    assert_eq!(sink.drain().last().unwrap(), "abX\ncdY");

    for _ in 0..8 {
        ui.event(&key("Enter", false, false));
    }
    ui.paint(vp);
    let r = ui.tree.rect(a);
    let c = Point::new(r.x + 5.0, r.y + 5.0);
    ui.event(&UiEvent::Wheel { pos: c, dx: 0.0, dy: 1000.0 });
    ui.event(&UiEvent::Wheel { pos: c, dx: 0.0, dy: -1000.0 });
    let list = ui.paint(vp);
    assert!(list.cmds().iter().any(|c| matches!(c, DrawCmd::ClipPush(_))));
}

#[test]
fn area_mouse_drag_and_double_click() {
    let (mut ui, _sink) = ui();
    let root = ui.root();
    let a = TextArea::add(&mut ui, root, "hello world", 300.0, 3, Callback::new(|s| s));
    let vp = Size::new(400.0, 300.0);
    ui.paint(vp);
    let r = ui.tree.rect(a);
    let y = r.y + 10.0;
    down(&mut ui, Point::new(r.x + 2.0, y));
    ui.event(&UiEvent::PointerMove { pos: Point::new(r.x + 290.0, y) });
    up(&mut ui, Point::new(r.x + 290.0, y));
    ui.event(&key("c", false, true));
    ui.event(&key("End", false, true));
    ui.event(&key("v", false, true));
    ui.paint(vp);
    // двойной клик по началу выделяет слово; печать заменяет его
    let p = Point::new(r.x + 12.0, y);
    down(&mut ui, p);
    up(&mut ui, p);
    down(&mut ui, p);
    up(&mut ui, p);
    ui.event(&UiEvent::Text("Z".into()));
}

#[test]
fn input_drag_double_click_escape() {
    let (mut ui, sink) = ui();
    let root = ui.root();
    let i = TextInput::add(&mut ui, root, "hello world", 300.0, Callback::new(|s| s));
    let vp = Size::new(400.0, 100.0);
    ui.paint(vp);
    let r = ui.tree.rect(i);
    let y = r.y + r.h / 2.0;
    let p = Point::new(r.x + 12.0, y);
    down(&mut ui, p);
    up(&mut ui, p);
    down(&mut ui, p);
    up(&mut ui, p);
    ui.event(&UiEvent::Text("Z".into()));
    assert_eq!(sink.drain().last().unwrap(), "Z world");

    // Esc снимает выделение: печать вставляется, а не заменяет
    ui.event(&key("a", false, true));
    ui.event(&key("Escape", false, false));
    ui.event(&UiEvent::Text("!".into()));
    assert_eq!(sink.drain().last().unwrap().matches('!').count(), 1);
    assert!(sink.is_empty());

    // drag выделяет до конца
    ui.paint(vp);
    down(&mut ui, Point::new(r.x + 2.0, y));
    ui.event(&UiEvent::PointerMove { pos: Point::new(r.x + 290.0, y) });
    up(&mut ui, Point::new(r.x + 290.0, y));
    ui.event(&UiEvent::Text("Q".into()));
    assert_eq!(sink.drain().last().unwrap(), "Q");
}

#[test]
fn input_scrolls_with_clip() {
    let (mut ui, _sink) = ui();
    let root = ui.root();
    let i = TextInput::add(&mut ui, root, "", 80.0, Callback::new(|s| s));
    let vp = Size::new(400.0, 100.0);
    ui.paint(vp);
    ui.event(&key("Tab", false, false));
    ui.event(&UiEvent::Text("очень длинная строка текста".into()));
    let list = ui.paint(vp);
    assert!(list.cmds().iter().any(|c| matches!(c, DrawCmd::ClipPush(_))));
    let _ = i;
}
