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

fn input_harness() -> Harness<String> {
    let mut h = Harness::<String>::new(320.0, 80.0);
    let root = h.ui.root();
    TextInput::add(&mut h.ui, root, "", 240.0, Callback::new(|s| s));
    h.frame();
    h
}

#[test]
fn recorded_session_replays_to_identical_frames() {
    let mut a = input_harness();
    a.start_recording();
    let target = a.ui.tree.rect(a.ui.root());
    a.advance(16);
    a.click_at(hey_core::Point::new(target.x + 20.0, target.y + 20.0));
    a.advance(120);
    a.type_text("Hello, \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}");
    a.advance(300);
    a.undo();
    a.checkpoint();
    let rec = a.stop_recording();
    let want = a.frame_hash();

    // Through the text format and back, into a fresh UI.
    let rec = hey_test::Recording::parse(&rec.to_text()).unwrap();
    let mut b = input_harness();
    b.replay(&rec).unwrap();
    assert_eq!(b.frame_hash(), want);
    assert_eq!(b.messages(), a.messages());
}

#[test]
fn replay_reports_a_diverging_frame() {
    let mut rec = hey_test::Recording::new();
    rec.push_event(0, hey_core::UiEvent::Text("x".into()));
    rec.push_hash(10, 1);
    let err = input_harness().replay(&rec).unwrap_err();
    assert_eq!((err.entry, err.ms, err.expected), (1, 10, 1));
}

/// A committed recording (written on first run, like goldens) stays replayable bit-exactly.
#[test]
fn committed_session_replays() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sessions/typing.hey-session");
    if !std::path::Path::new(path).exists() || std::env::var_os("HEY_UPDATE_GOLDEN").is_some() {
        let mut h = input_harness();
        h.start_recording();
        h.advance(10);
        h.click_at(hey_core::Point::new(30.0, 30.0));
        h.advance(50);
        h.type_text("Hey");
        h.checkpoint();
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
        h.stop_recording().save(path).unwrap();
    }
    input_harness().replay_file(path).unwrap();
}

#[test]
fn inspector_is_off_by_default_and_diffs_frames() {
    use hey_style::Variant;
    use hey_widgets::Button;
    let mut h = Harness::<String>::new(300.0, 120.0);
    let root = h.ui.root();
    let b = Button::add(&mut h.ui, root, "Save", Variant::Primary, Callback::new(|_| String::new()));
    h.frame();
    let plain = h.ui.paint(h.viewport);
    assert!(!h.ui.inspector_enabled());
    assert!(h.ui.inspector_diff().is_empty());

    h.ui.set_inspector(true);
    let with = h.ui.paint(h.viewport);
    assert!(with.len() > plain.len(), "overlay adds commands");
    assert!(h.ui.inspector_snapshot());
    assert!(h.ui.inspector_diff().is_empty());

    h.click(b);
    let info = h.ui.inspect_node(b);
    assert!(info[0].contains("Save"), "{info:?}");
    assert!(!h.ui.inspector_diff().is_empty(), "hover/press changes the frame");
    assert!(h.ui.dump_tree().contains("Save"));

    h.ui.set_inspector(false);
    assert_eq!(h.ui.paint(h.viewport).len(), h.ui.paint(h.viewport).len());
    assert!(h.ui.inspector_diff().is_empty());
}

#[test]
fn layout_matrix_over_text_scale_and_scripts() {
    use hey_style::Variant;
    use hey_test::LayoutIssueKind;
    use hey_widgets::{Button, Label};

    // Latin and Arabic labels: the matrix must stay free of clipping and overlap.
    for text in ["Save changes", "\u{62d}\u{641}\u{638} \u{627}\u{644}\u{62a}\u{63a}\u{64a}\u{64a}\u{631}\u{627}\u{62a}"] {
        let mut h = Harness::<String>::new(640.0, 240.0);
        let root = h.ui.root();
        Label::add(&mut h.ui, root, text);
        Button::add(&mut h.ui, root, text, Variant::Primary, Callback::new(|_| String::new()));
        TextInput::add(&mut h.ui, root, "", 200.0, Callback::new(|s| s));
        let mut seen = 0;
        h.text_scale_matrix(&[100, 150, 200], |h| {
            seen += 1;
            let issues = h.audit_layout();
            assert!(issues.iter().all(|i| i.kind != LayoutIssueKind::Overlap), "{text}: {issues:?}");
        });
        assert_eq!(seen, 3);
    }
}
