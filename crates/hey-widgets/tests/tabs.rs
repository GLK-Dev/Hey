// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_layout::Style;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{Panel, PanelKind, Tabs, Ui};

fn key(c: &str) -> UiEvent {
    UiEvent::KeyDown { code: c.into(), mods: Modifiers::default() }
}

#[test]
fn tabs_keyboard_click_and_merge() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let sink = MsgSink::new();
    let mut ui: Ui<usize> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], sink.clone());
    let root = ui.root();
    let tabs = Tabs::add(&mut ui, root, &["Один", "Два", "Три"], 0, Callback::new(|i| i));
    Panel::add_attached(&mut ui, root, Style::column().grow(1.0), PanelKind::Panel);
    let vp = Size::new(400.0, 200.0);
    let dl = ui.paint(vp);
    assert!(!dl.cmds().iter().any(|c| matches!(c, DrawCmd::Rect { border: Some(_), .. })));

    ui.event(&key("Tab"));
    assert_eq!(ui.focused(), Some(tabs));
    ui.event(&key("ArrowRight"));
    ui.event(&key("ArrowRight"));
    ui.event(&key("ArrowRight"));
    ui.event(&key("Home"));
    ui.event(&key("End"));
    assert_eq!(sink.drain(), [1, 2, 0, 2]);

    let r = ui.tree.rect(tabs);
    let p = Point::new(r.x + 5.0, r.y + r.h / 2.0); // first tab
    ui.event(&UiEvent::PointerMove { pos: p });
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(sink.drain(), [0]);
    ui.paint(vp);
}
