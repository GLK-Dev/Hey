// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Color, Modifiers, MouseButton, MsgSink, NodeId, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_layout::Style;
use hey_style::{Theme, Variant};
use hey_text::Fonts;
use hey_widgets::{Button, Label, Ui};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Ok,
    Cancel,
}

const VP: Size = Size { w: 400.0, h: 200.0 };

fn setup() -> (Ui<Msg>, MsgSink<Msg>, NodeId, NodeId, NodeId) {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    let mut ui = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], sink.clone());
    let root = ui.root();
    let row = ui.add_container(root, Style::row().gap(10.0));
    let label = Label::add(&mut ui, row, "Привет");
    let ok = Button::add(&mut ui, row, "OK", Variant::Primary, Callback::new(|_| Msg::Ok));
    let cancel = Button::add(&mut ui, row, "Отмена", Variant::Secondary, Callback::new(|_| Msg::Cancel));
    ui.paint(VP);
    (ui, sink, label, ok, cancel)
}

fn center(ui: &Ui<Msg>, id: NodeId) -> Point {
    let r = ui.tree.rect(id);
    Point::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn key(code: &str) -> UiEvent {
    UiEvent::KeyDown { code: code.into(), mods: Modifiers::default() }
}

#[test]
fn click_sends_message_only_on_release_inside() {
    let (mut ui, sink, _, ok, cancel) = setup();
    let p = center(&ui, ok);
    ui.event(&UiEvent::PointerMove { pos: p });
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    assert!(sink.is_empty());
    assert_eq!(ui.focused(), Some(ok));
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(sink.drain(), [Msg::Ok]);

    // pressed on OK, released over "Отмена" — no click
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: center(&ui, cancel), button: MouseButton::Left });
    assert!(sink.is_empty());
}

#[test]
fn keyboard_tab_enter_space() {
    let (mut ui, sink, label, ok, cancel) = setup();
    assert!(!ui.event(&key("Enter")), "Enter does nothing without focus");
    ui.event(&key("Tab"));
    assert_eq!(ui.focused(), Some(ok));
    ui.event(&key("Tab"));
    assert_eq!(ui.focused(), Some(cancel));
    ui.event(&key(" "));
    ui.event(&key("Enter"));
    assert_eq!(sink.drain(), [Msg::Cancel, Msg::Cancel]);
    let shift = UiEvent::KeyDown { code: "Tab".into(), mods: Modifiers { shift: true, ..Default::default() } };
    ui.event(&shift);
    assert_eq!(ui.focused(), Some(ok));
    assert_ne!(ui.focused(), Some(label), "Label is not focusable");
}

#[test]
fn disabled_button_ignores_input() {
    let (mut ui, sink, _, ok, _) = setup();
    ui.set_disabled(ok, true);
    let p = center(&ui, ok);
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert!(sink.is_empty());
    assert_eq!(ui.focused(), None);
}

#[test]
fn paint_colors_follow_state() {
    let (mut ui, _, _, ok, _) = setup();
    let t = Theme::dark();
    let fill_of = |ui: &mut Ui<Msg>| -> Color {
        ui.paint(VP);
        ui.tick(10_000.0); // let the state crossfade finish
        let dl = ui.paint(VP);
        dl.cmds()
            .iter()
            .find_map(|c| match c {
                DrawCmd::Rect { fill, border: None, .. } if *fill != Color::TRANSPARENT => Some(*fill),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(fill_of(&mut ui), t.palette.accent);
    let p = center(&ui, ok);
    ui.event(&UiEvent::PointerMove { pos: p });
    assert_eq!(fill_of(&mut ui), t.button_bg(Variant::Primary, hey_style::WidgetState::HOVERED));
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    assert_eq!(fill_of(&mut ui), t.palette.accent_dark);

    let dl = ui.paint(VP);
    assert!(dl.cmds().iter().any(|c| matches!(c, DrawCmd::Text(_))));
    // focus ring is painted
    assert!(dl.cmds().iter().any(|c| matches!(c, DrawCmd::Rect { fill, border: Some(b), .. }
        if *fill == Color::TRANSPARENT && b.color == t.palette.focus)));
}

#[test]
fn layout_places_children_in_row() {
    let (ui, _, label, ok, cancel) = setup();
    let (a, b, c) = (ui.tree.rect(label), ui.tree.rect(ok), ui.tree.rect(cancel));
    assert!(a.w > 0.0 && b.x >= a.right() + 10.0 - 0.01 && c.x >= b.right() + 10.0 - 0.01);
}
