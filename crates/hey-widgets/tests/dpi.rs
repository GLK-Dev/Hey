// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_style::Theme;
use hey_style::Variant;
use hey_text::Fonts;
use hey_widgets::{Button, Ui};

fn ui() -> Ui<&'static str> {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let mut ui = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new());
    let root = ui.root();
    Button::add(&mut ui, root, "OK", Variant::Primary, Callback::new(|()| "clicked"));
    ui
}

fn first_rect(list: &hey_draw::DisplayList) -> hey_core::Rect {
    list.cmds()
        .iter()
        .find_map(|c| match c {
            DrawCmd::Rect { rect, .. } => Some(*rect),
            _ => None,
        })
        .unwrap()
}

#[test]
fn paint_output_is_physical_and_layout_logical() {
    let mut a = ui();
    let r1 = first_rect(&a.paint(Size::new(400.0, 200.0)));
    let mut b = ui();
    b.set_scale(2.0);
    let r2 = first_rect(&b.paint(Size::new(800.0, 400.0)));
    assert_eq!((r2.x, r2.y, r2.w, r2.h), (r1.x * 2.0, r1.y * 2.0, r1.w * 2.0, r1.h * 2.0));
    // Layout itself stays logical.
    let id = b.tree.children(b.root())[0];
    assert_eq!(b.tree.rect(id).w, r1.w);
}

#[test]
fn physical_click_hits_scaled_widget() {
    let mut u = ui();
    u.event(&UiEvent::ScaleFactor(2.0));
    assert_eq!(u.scale(), 2.0);
    let list = u.paint(Size::new(800.0, 400.0));
    let r = first_rect(&list);
    let p = Point::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
    u.event(&UiEvent::PointerMove { pos: p });
    u.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    u.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(u.focused(), Some(u.tree.children(u.root())[0]));
}

#[test]
fn invalid_scale_is_ignored() {
    let mut u = ui();
    u.set_scale(0.0);
    u.set_scale(f64::NAN);
    assert_eq!(u.scale(), 1.0);
}
