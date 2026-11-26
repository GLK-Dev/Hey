// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, MsgSink, Point, Size, UiEvent};
use hey_draw::DrawCmd;
use hey_style::{Theme, Variant};
use hey_text::Fonts;
use hey_widgets::{Button, Ui};
use std::sync::Arc;

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn setup(theme: Theme) -> Ui<()> {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let mut ui = Ui::new(theme, Arc::new(f), vec![onest], MsgSink::new());
    let root = ui.root();
    Button::add(&mut ui, root, "Go", Variant::Primary, Callback::new(|_| ()));
    ui.paint(VIEW);
    ui
}

fn first_fill(ui: &mut Ui<()>) -> hey_core::Color {
    match ui.paint(VIEW).cmds().iter().find(|c| matches!(c, DrawCmd::Rect { .. })) {
        Some(DrawCmd::Rect { fill, .. }) => *fill,
        _ => panic!("no rect"),
    }
}

fn hover(ui: &mut Ui<()>) {
    let r = ui.tree.rect(ui.tree.children(ui.root())[0]);
    ui.event(&UiEvent::PointerMove { pos: Point::new(r.x + 5.0, r.y + 5.0) });
}

#[test]
fn hover_crossfades_button_fill() {
    let mut ui = setup(Theme::dark());
    let idle = first_fill(&mut ui);
    hover(&mut ui);
    let start = first_fill(&mut ui);
    assert!(ui.animating());
    assert_eq!(start, idle, "fade starts from the old color");
    ui.tick(30.0);
    let mid = first_fill(&mut ui);
    assert!(ui.tick(0.0));
    ui.tick(10_000.0);
    assert!(!ui.animating());
    let end = first_fill(&mut ui);
    assert_ne!(end, idle);
    assert!(mid != idle && mid != end, "mid-fade color differs from both ends");
}

#[test]
fn reduced_motion_does_not_animate() {
    let mut ui = setup(Theme::dark().reduced_motion());
    let idle = first_fill(&mut ui);
    hover(&mut ui);
    let now = first_fill(&mut ui);
    assert!(!ui.animating());
    assert_ne!(now, idle);
}
