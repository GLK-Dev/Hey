// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{Checkbox, ProgressBar, Radio, Slider, Toggle, Ui};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug, PartialEq)]
enum M {
    Check(bool),
    Tog(bool),
    Radio(usize),
    Slide(f32),
}

fn key(c: &str) -> UiEvent {
    UiEvent::KeyDown { code: c.into(), mods: Modifiers::default() }
}

#[test]
fn controls_basic() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    let mut ui: Ui<M> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], sink.clone());
    let root = ui.root();
    let cb = Checkbox::add(&mut ui, root, "Флажок", false, Callback::new(M::Check));
    let _tg = Toggle::add(&mut ui, root, "Тумблер", false, Callback::new(M::Tog));
    let group = Rc::new(Cell::new(None));
    let _r0 = Radio::add(&mut ui, root, "Один", 0, group.clone(), Callback::new(M::Radio));
    let r1 = Radio::add(&mut ui, root, "Два", 1, group.clone(), Callback::new(M::Radio));
    let sl = Slider::add(&mut ui, root, 0.0, 100.0, 0.0, 200.0, Callback::new(M::Slide));
    let pb = ProgressBar::add(&mut ui, root, 0.25, 200.0);
    let vp = Size::new(400.0, 400.0);
    ui.paint(vp);

    // checkbox: click
    let r = ui.tree.rect(cb);
    let p = Point::new(r.x + 3.0, r.y + 3.0);
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(sink.drain(), [M::Check(true)]);
    ui.event(&key(" "));
    assert_eq!(sink.drain(), [M::Check(false)]);

    // toggle: Tab, Space
    ui.event(&key("Tab"));
    ui.event(&key(" "));
    assert_eq!(sink.drain(), [M::Tog(true)]);

    // radio: choosing, and choosing the same one again sends no message
    ui.event(&key("Tab"));
    ui.event(&key(" "));
    assert_eq!(sink.drain(), [M::Radio(0)]);
    ui.event(&key(" "));
    assert!(sink.is_empty());
    let r = ui.tree.rect(r1);
    let p = Point::new(r.x + 3.0, r.y + 3.0);
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(sink.drain(), [M::Radio(1)]);
    assert_eq!(group.get(), Some(1));

    // slider: click in the middle, drag to the end, keys
    let r = ui.tree.rect(sl);
    let mid = Point::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
    ui.event(&UiEvent::PointerDown { pos: mid, button: MouseButton::Left });
    let M::Slide(v) = sink.drain().pop().unwrap() else { panic!() };
    assert!((v - 50.0).abs() < 1.0, "{v}");
    let end = Point::new(r.x + r.w + 50.0, mid.y);
    ui.event(&UiEvent::PointerMove { pos: end });
    ui.event(&UiEvent::PointerUp { pos: end, button: MouseButton::Left });
    assert_eq!(sink.drain().pop(), Some(M::Slide(100.0)));
    ui.event(&key("Home"));
    assert_eq!(sink.drain(), [M::Slide(0.0)]);
    ui.event(&key("ArrowRight"));
    assert_eq!(sink.drain(), [M::Slide(5.0)]);

    ProgressBar::set(&mut ui, pb, 2.0);
    ui.paint(vp);
}
