// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{List, Role, Ui, View};

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

    // click on the first row
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
    assert!(sink.is_empty(), "selection does not change on the last row");

    // the selected row is scrolled into view: not all 50 rows are drawn
    let list = ui.paint(vp);
    let texts = list.cmds().iter().filter(|c| matches!(c, DrawCmd::Text(_))).count();
    assert!(texts > 0 && texts <= 7, "only visible rows are drawn: {texts}");

    ui.event(&UiEvent::Wheel { pos: p, dx: 0.0, dy: 100000.0 });
    ui.event(&key("Home"));
    assert_eq!(sink.drain(), [0]);
    ui.paint(vp);
}

fn list_view(ids: Vec<u32>) -> View<usize> {
    View::leaf("list", ids.clone(), move |ui, p| {
        let rows = ids.iter().map(|i| (*i, format!("Row {i}"))).collect();
        List::add_keyed(ui, p, rows, 200.0, 5, Callback::new(|i| i))
    })
}

fn list_value(ui: &Ui<usize>) -> String {
    ui.a11y_tree().nodes.into_iter().find(|n| n.a11y.role == Role::List).map(|n| n.a11y.value.unwrap_or_default()).unwrap_or_default()
}

#[test]
fn keyed_selection_follows_row_across_rebuild() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let mut ui: Ui<usize> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new());
    let root = ui.root();
    ui.reconcile(root, vec![list_view(vec![10, 20, 30])]);
    ui.paint(Size::new(400.0, 300.0));
    let r = ui.tree.rect(ui.tree.children(root)[0]);
    let p = Point::new(r.x + 20.0, r.y + 40.0);
    ui.event(&UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    ui.event(&UiEvent::PointerUp { pos: p, button: MouseButton::Left });
    assert_eq!(list_value(&ui), "Row 20");

    // A row is inserted in front: key 20 moves from index 1 to 2 and stays selected.
    ui.reconcile(root, vec![list_view(vec![5, 10, 20, 30])]);
    assert_eq!(list_value(&ui), "Row 20");
    // The selected row is removed: nothing is selected.
    ui.reconcile(root, vec![list_view(vec![5, 10, 30])]);
    assert_eq!(list_value(&ui), "");
}
