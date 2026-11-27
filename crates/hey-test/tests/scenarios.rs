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

#[test]
fn hidpi_screenshot_at_scale_two() {
    let mut h = Harness::<String>::new(640.0, 160.0);
    h.set_scale(2.0);
    let root = h.ui.root();
    let input = TextInput::add(&mut h.ui, root, "", 240.0, Callback::new(|s| s));
    h.frame();
    // Pointer coordinates are physical; click() uses the logical rect, so scale it.
    let r = h.ui.tree.rect(input);
    h.click_at(hey_core::Point::new((r.x + r.w * 0.5) * 2.0, (r.y + r.h * 0.5) * 2.0));
    assert_eq!(h.ui.focused(), Some(input));
    h.type_text("Hello");
    h.screenshot(GOLDEN, "typed_2x");
}

#[test]
fn audit_passes_for_named_controls_and_flags_unnamed_ones() {
    use hey_style::Variant;
    use hey_test::IssueKind;
    use hey_widgets::Button;

    let mut h = Harness::<String>::new(400.0, 200.0);
    let root = h.ui.root();
    Button::add(&mut h.ui, root, "Save", Variant::Primary, Callback::new(|_| String::new()));
    TextInput::add(&mut h.ui, root, "", 200.0, Callback::new(|s| s));
    let issues = h.audit();
    assert!(issues.iter().all(|i| i.kind == IssueKind::Unnamed), "unexpected findings: {issues:?}");
    let before = issues.len();

    Button::add(&mut h.ui, root, "", Variant::Primary, Callback::new(|_| String::new()));
    let issues = h.audit();
    assert_eq!(issues.len(), before + 1, "{issues:?}");
    assert!(issues.iter().any(|i| i.kind == IssueKind::Unnamed && i.label.is_empty()));
}
