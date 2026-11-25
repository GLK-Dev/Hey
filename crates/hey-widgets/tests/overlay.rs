// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Rect, Size, UiEvent};
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{Dropdown, Menu, Modal, Tooltip, Ui};
use std::rc::Rc;

#[derive(Debug, PartialEq)]
enum M {
    Pick(usize),
    Modal(usize),
}

fn key(c: &str) -> UiEvent {
    UiEvent::KeyDown { code: c.into(), mods: Modifiers::default() }
}

fn click(ui: &mut Ui<M>, p: Point) {
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
}

fn setup() -> (Ui<M>, MsgSink<M>) {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    (Ui::new(Theme::dark(), Rc::new(f), vec![onest], sink.clone()), sink)
}

fn items() -> Vec<String> {
    ["Раз", "Два", "Три"].iter().map(|s| s.to_string()).collect()
}

#[test]
fn dropdown_menu_select() {
    let (mut ui, sink) = setup();
    let root = ui.root();
    let dd = Dropdown::add(&mut ui, root, items(), 0, 160.0, Callback::new(M::Pick));
    let vp = Size::new(400.0, 300.0);
    ui.paint(vp);
    let r: Rect = ui.tree.rect(dd);

    click(&mut ui, Point::new(r.x + 5.0, r.y + 5.0));
    assert_eq!(ui.overlay_count(), 1, "клик открывает меню");
    ui.paint(vp);

    // стрелка вниз + Enter
    ui.event(&key("ArrowDown"));
    ui.event(&key("Enter"));
    assert_eq!(sink.drain(), [M::Pick(1)]);
    assert_eq!(ui.overlay_count(), 0);

    // клик снаружи закрывает без выбора
    click(&mut ui, Point::new(r.x + 5.0, r.y + 5.0));
    assert_eq!(ui.overlay_count(), 1);
    click(&mut ui, Point::new(390.0, 290.0));
    assert_eq!(ui.overlay_count(), 0);
    assert!(sink.is_empty());

    // Esc
    click(&mut ui, Point::new(r.x + 5.0, r.y + 5.0));
    ui.event(&key("Escape"));
    assert_eq!(ui.overlay_count(), 0);
    assert!(sink.is_empty());
}

#[test]
fn menu_click_and_tooltip() {
    let (mut ui, sink) = setup();
    let vp = Size::new(400.0, 300.0);
    ui.paint(vp);
    Menu::open(&mut ui, Point::new(10.0, 10.0), items(), Callback::new(M::Pick));
    ui.paint(vp);
    // третья строка
    click(&mut ui, Point::new(20.0, 10.0 + 1.0 + 2.5 * 24.0));
    let got = sink.drain();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(ui.overlay_count(), 0);

    let id = Tooltip::show(&mut ui, Rect::new(50.0, 50.0, 20.0, 20.0), "Подсказка");
    ui.paint(vp);
    assert_eq!(ui.overlay_count(), 1);
    ui.close_overlay(id);
    ui.paint(vp);
    assert_eq!(ui.overlay_count(), 0);
}

#[test]
fn modal_traps_input() {
    let (mut ui, sink) = setup();
    let vp = Size::new(500.0, 400.0);
    ui.paint(vp);
    Modal::open(&mut ui, "Заголовок", "Удалить файл?", vec!["Да".into(), "Нет".into()], Callback::new(M::Modal));
    ui.paint(vp);
    assert_eq!(ui.overlay_count(), 1);

    ui.event(&key("Tab"));
    assert_eq!(ui.overlay_count(), 1, "Tab заперт");
    ui.event(&key("ArrowRight"));
    ui.event(&key("Enter"));
    assert_eq!(sink.drain(), [M::Modal(1)]);
    assert_eq!(ui.overlay_count(), 0);

    Modal::open(&mut ui, "З", "Т", vec!["Да".into()], Callback::new(M::Modal));
    ui.paint(vp);
    ui.event(&key("Escape"));
    assert_eq!(ui.overlay_count(), 0);
    assert!(sink.is_empty());
}
