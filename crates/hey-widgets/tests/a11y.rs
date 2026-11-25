// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, MsgSink, Size};
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{A11yAction, Checkbox, Label, Role, Ui};

fn ui() -> Ui<bool> {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new())
}

#[test]
fn tree_describes_widgets_and_focus() {
    let mut ui = ui();
    let root = ui.root();
    Label::add(&mut ui, root, "Hello");
    Checkbox::add(&mut ui, root, "Agree", false, Callback::new(|b| b));
    ui.paint(Size::new(300.0, 100.0));

    let t = ui.a11y_tree();
    assert_eq!(t.nodes[0].id, t.root);
    assert_eq!(t.nodes[0].children.len(), 2);
    let find = |role| t.nodes.iter().find(|n| n.a11y.role == role).unwrap();
    assert_eq!(find(Role::Label).a11y.label, "Hello");
    let cb = find(Role::CheckBox);
    assert_eq!((cb.a11y.label.as_str(), cb.a11y.toggled, cb.focusable), ("Agree", Some(false), true));
    assert!(cb.rect.w > 0.0);
    assert_eq!(t.focus, None);

    assert!(ui.a11y_action(A11yAction::Focus(cb.id)));
    assert_eq!(ui.a11y_tree().focus, Some(cb.id));
}

#[test]
fn activate_action_toggles_and_rejects_stale_ids() {
    let mut ui = ui();
    let root = ui.root();
    Checkbox::add(&mut ui, root, "Agree", false, Callback::new(|b| b));
    ui.paint(Size::new(300.0, 100.0));
    let id = ui.a11y_tree().nodes.iter().find(|n| n.a11y.role == Role::CheckBox).unwrap().id;

    assert!(ui.a11y_action(A11yAction::Activate(id)));
    let after = ui.a11y_tree();
    assert_eq!(after.nodes.iter().find(|n| n.id == id).unwrap().a11y.toggled, Some(true));

    assert!(!ui.a11y_action(A11yAction::Activate(0)), "0 is not a node id");
    assert!(!ui.a11y_action(A11yAction::Focus(u64::MAX)), "overlay-range ids are not nodes");
    assert!(!ui.a11y_action(A11yAction::Activate(id + (1 << 32))), "wrong generation");
}
