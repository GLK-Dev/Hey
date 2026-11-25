// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MsgSink, Size, UiEvent};
use hey_layout::Style;
use hey_style::{Theme, Variant};
use hey_text::Fonts;
use hey_widgets::{Button, Ui};

fn tab(shift: bool) -> UiEvent {
    UiEvent::KeyDown { code: "Tab".into(), mods: Modifiers { shift, ..Modifiers::default() } }
}

#[test]
fn focus_trap_and_programmatic_focus() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let mut ui: Ui<u8> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new());
    let root = ui.root();
    let a = Button::add(&mut ui, root, "A", Variant::Secondary, Callback::new(|_| 0));
    let scope = ui.add_container(root, Style::column());
    let b = Button::add(&mut ui, scope, "B", Variant::Secondary, Callback::new(|_| 1));
    let c = Button::add(&mut ui, scope, "C", Variant::Secondary, Callback::new(|_| 2));
    ui.paint(Size::new(300.0, 300.0));

    assert!(ui.focus(a));
    assert_eq!(ui.focused(), Some(a));

    // the trap moves focus inside and closes the Tab cycle
    ui.trap_focus(Some(scope));
    assert_eq!(ui.focused(), Some(b));
    ui.event(&tab(false));
    assert_eq!(ui.focused(), Some(c));
    ui.event(&tab(false));
    assert_eq!(ui.focused(), Some(b));
    ui.event(&tab(true));
    assert_eq!(ui.focused(), Some(c));
    assert!(!ui.focus(a), "focus cannot be set outside the trap");

    // removing the trap restores normal traversal
    ui.trap_focus(None);
    ui.event(&tab(false));
    assert_eq!(ui.focused(), Some(a));
    ui.blur();
    assert_eq!(ui.focused(), None);
}
