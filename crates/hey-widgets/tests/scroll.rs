// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{MouseButton, MsgSink, NodeId, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_layout::Style;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{Label, Ui};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn setup() -> (Ui<()>, NodeId) {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let mut ui = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new());
    let root = ui.root();
    let sv = ui.add_container(root, Style::column().size(200.0, 100.0).scroll(false, true));
    for i in 0..20 {
        Label::add(&mut ui, sv, &format!("Row {i}"));
    }
    ui.paint(VIEW);
    (ui, sv)
}

fn wheel(ui: &mut Ui<()>, dy: f32) -> bool {
    let r = ui.tree.rect(ui.tree.children(ui.root())[0]);
    ui.event(&UiEvent::Wheel { pos: Point::new(r.x + 20.0, r.y + 20.0), dx: 0.0, dy })
}

#[test]
fn wheel_on_child_scrolls_container() {
    let (mut ui, sv) = setup();
    assert_eq!(ui.tree.scroll(sv).y, 0.0);
    assert!(wheel(&mut ui, -30.0));
    assert_eq!(ui.tree.scroll(sv).y, 30.0);
    // Clamped at the top and at the bottom.
    assert!(wheel(&mut ui, 100.0));
    assert_eq!(ui.tree.scroll(sv).y, 0.0);
    assert!(!wheel(&mut ui, 100.0));
    assert!(wheel(&mut ui, -100_000.0));
    assert_eq!(ui.tree.scroll(sv).y, ui.tree.max_scroll(sv).y);
}

#[test]
fn content_is_clipped_and_scrollbar_drawn_on_top() {
    let (mut ui, _) = setup();
    let list = ui.paint(VIEW);
    let cmds = list.cmds();
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::ClipPush(_))));
    let pop = cmds.iter().rposition(|c| matches!(c, DrawCmd::ClipPop)).unwrap();
    // The thumb is the last command: drawn after the clip is closed.
    assert!(pop < cmds.len() - 1);
    assert!(matches!(cmds.last(), Some(DrawCmd::Rect { .. })));
}

#[test]
fn thumb_drag_scrolls_proportionally() {
    let (mut ui, sv) = setup();
    let r = ui.tree.rect(sv);
    let max = ui.tree.max_scroll(sv).y;
    assert!(max > 0.0);
    // Thumb sits at the right edge, at the top of the track.
    let grab = Point::new(r.x + r.w - 6.0, r.y + 6.0);
    assert!(ui.event(&UiEvent::PointerDown { pos: grab, button: MouseButton::Left }));
    // Drag well past the bottom: scroll is clamped to the maximum.
    ui.event(&UiEvent::PointerMove { pos: Point::new(grab.x, r.y + r.h + 500.0) });
    assert_eq!(ui.tree.scroll(sv).y, max);
    ui.event(&UiEvent::PointerMove { pos: Point::new(grab.x, r.y - 500.0) });
    assert_eq!(ui.tree.scroll(sv).y, 0.0);
    ui.event(&UiEvent::PointerUp { pos: grab, button: MouseButton::Left });
    // After release, moves no longer scroll.
    ui.event(&UiEvent::PointerMove { pos: Point::new(grab.x, r.y + 80.0) });
    assert_eq!(ui.tree.scroll(sv).y, 0.0);
}

#[test]
fn track_click_pages() {
    let (mut ui, sv) = setup();
    let r = ui.tree.rect(sv);
    // Below the thumb on the track: one page down.
    let below = Point::new(r.x + r.w - 6.0, r.y + r.h - 6.0);
    assert!(ui.event(&UiEvent::PointerDown { pos: below, button: MouseButton::Left }));
    ui.event(&UiEvent::PointerUp { pos: below, button: MouseButton::Left });
    assert!(ui.tree.scroll(sv).y > 0.0);
}
