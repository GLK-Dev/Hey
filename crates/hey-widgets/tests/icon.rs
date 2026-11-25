// Copyright (c) Vitaly Golik (mjojo)

use std::sync::Arc;

use hey_core::{MsgSink, Size};
use hey_draw::DrawCmd;
use hey_layout::Style;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{Icon, Ui};

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><rect width="8" height="8" fill="#fff"/></svg>"##;

#[test]
fn icon_emits_image_and_reuses_store_until_resized() {
    let mut ui: Ui<()> = Ui::new(Theme::dark(), Arc::new(Fonts::new()), vec![], MsgSink::new());
    ui.env.icons.borrow_mut().cache.add_source("sq", SVG.to_vec());
    let root = ui.root();
    let row = ui.add_container(root, Style::row());
    let icon = Icon::add(&mut ui, row, "sq", 16.0);
    let list = ui.paint(Size { w: 100.0, h: 100.0 });
    let r = ui.tree.rect(icon);
    assert_eq!((r.w, r.h), (16.0, 16.0));
    assert!(list.cmds().iter().any(|c| matches!(c, DrawCmd::Image { rect, .. } if rect.w == 16.0 && rect.h == 16.0)));
    let store = ui.env.icons.borrow().images();
    ui.paint(Size { w: 100.0, h: 100.0 });
    assert!(Arc::ptr_eq(&store, &ui.env.icons.borrow().images()), "repaint must not re-rasterize");
}

#[test]
fn missing_icon_draws_nothing() {
    let mut ui: Ui<()> = Ui::new(Theme::dark(), Arc::new(Fonts::new()), vec![], MsgSink::new());
    let root = ui.root();
    Icon::add(&mut ui, root, "nope", 16.0);
    let list = ui.paint(Size { w: 100.0, h: 100.0 });
    assert!(!list.cmds().iter().any(|c| matches!(c, DrawCmd::Image { .. })));
}
