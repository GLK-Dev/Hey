// Copyright (c) Vitaly Golik (mjojo)

use hey_core::Callback;
use hey_test::Harness;
use hey_widgets::TextInput;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");

#[test]
fn click_type_undo_screenshot() {
    let mut h = Harness::<String>::new(320.0, 80.0);
    let root = h.ui.root();
    let input = TextInput::add(&mut h.ui, root, "", 240.0, Callback::new(|s| s));
    h.frame();

    h.click(input);
    assert_eq!(h.ui.focused(), Some(input));

    h.type_text("Hello");
    assert_eq!(h.messages().last().unwrap(), "Hello");
    h.screenshot(GOLDEN, "typed");

    h.undo();
    assert_eq!(h.messages().last().unwrap(), "");
    h.screenshot(GOLDEN, "undone");

    h.redo();
    assert_eq!(h.messages().last().unwrap(), "Hello");
}

#[test]
fn rendering_is_deterministic() {
    let mut h = Harness::<String>::new(200.0, 60.0);
    let root = h.ui.root();
    TextInput::add(&mut h.ui, root, "same", 150.0, Callback::new(|s| s));
    assert!(h.render().data == h.render().data);
}
