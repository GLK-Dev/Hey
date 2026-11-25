// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MsgSink, Size, UiEvent};
use hey_layout::Style;
use hey_style::{Theme, Variant};
use hey_text::Fonts;
use hey_widgets::{Button, TextInput, Ui, View};

#[derive(Debug, PartialEq)]
enum M {
    Text(String),
    Click,
}

fn setup() -> Ui<M> {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new())
}

fn btn(key: &'static str, label: &'static str) -> View<M> {
    View::leaf(key, label, move |ui, p| Button::add(ui, p, label, Variant::Secondary, Callback::new(|_| M::Click)))
}

fn input(key: &'static str, text: &'static str) -> View<M> {
    View::leaf(key, (), move |ui, p| TextInput::add(ui, p, text, 200.0, Callback::new(M::Text)))
}

#[test]
fn reconcile_keeps_reorders_removes() {
    let mut ui = setup();
    let root = ui.root();
    ui.reconcile(root, vec![btn("a", "A"), btn("b", "B"), btn("c", "C")]);
    let ids: Vec<_> = ui.tree.children(root).to_vec();
    assert_eq!(ids.len(), 3);

    // те же свойства, другой порядок, "b" удалён, "d" добавлен
    ui.reconcile(root, vec![btn("c", "C"), btn("a", "A"), btn("d", "D")]);
    let now = ui.tree.children(root).to_vec();
    assert_eq!(now.len(), 3);
    assert_eq!(now[0], ids[2], "c сохранён и первый");
    assert_eq!(now[1], ids[0], "a сохранён");
    assert!(!ids.contains(&now[2]));

    // смена свойств → пересоздание
    ui.reconcile(root, vec![btn("c", "C2"), btn("a", "A"), btn("d", "D")]);
    let again = ui.tree.children(root).to_vec();
    assert_ne!(again[0], now[0]);
    assert_eq!(again[1], now[1]);
    ui.paint(Size::new(400.0, 300.0));
}

#[test]
fn reconcile_preserves_widget_state_and_focus() {
    let mut ui = setup();
    let root = ui.root();
    let col = || Style::column();
    ui.reconcile(root, vec![View::container("col", col(), vec![input("in", "привет мир")])]);
    ui.paint(Size::new(400.0, 300.0));
    let c = ui.tree.children(root)[0];
    let id = ui.tree.children(c)[0];
    assert!(ui.focus(id));
    ui.event(&UiEvent::KeyDown { code: "End".into(), mods: Modifiers::default() });

    // тот же ключ, другие свойства (хэш изменён) → новый узел, но каретка и фокус сохраняются
    let changed = View::leaf("in", 1u8, |ui: &mut Ui<M>, p| TextInput::add(ui, p, "привет мир", 200.0, Callback::new(M::Text)));
    ui.reconcile(root, vec![View::container("col", col(), vec![changed])]);
    assert_eq!(ui.tree.children(root)[0], c, "контейнер сохранён");
    let id2 = ui.tree.children(c)[0];
    assert_ne!(id2, id);
    assert_eq!(ui.focused(), Some(id2));
    ui.paint(Size::new(400.0, 300.0));
}
