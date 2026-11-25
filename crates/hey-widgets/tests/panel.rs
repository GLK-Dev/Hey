// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Insets, MsgSink, Size};
use hey_draw::DrawCmd;
use hey_layout::Style;
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::{Label, Panel, PanelKind, Ui};

#[test]
fn border_is_part_of_padding_and_paints_before_children() {
    let mut f = Fonts::new();
    let onest = f.add_file(concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf")).unwrap();
    let mut ui: Ui<()> = Ui::new(Theme::dark(), std::sync::Arc::new(f), vec![onest], MsgSink::new());
    let root = ui.root();
    let style = Style::column().padding(Insets::symmetric(4.0, 4.0));
    let panel = Panel::add(&mut ui, root, style, PanelKind::Window);
    let label = Label::add(&mut ui, panel, "Окно");
    let dl = ui.paint(Size::new(300.0, 200.0));

    let (p, l) = (ui.tree.rect(panel), ui.tree.rect(label));
    assert!((l.x - p.x - 5.0).abs() < 0.01 && (l.y - p.y - 5.0).abs() < 0.01, "{p:?} {l:?}");
    let cmds = dl.cmds();
    let rect_i = cmds.iter().position(|c| matches!(c, DrawCmd::Rect { border: Some(_), .. })).unwrap();
    let text_i = cmds.iter().position(|c| matches!(c, DrawCmd::Text(_))).unwrap();
    assert!(rect_i < text_i);
    let t = Theme::dark();
    assert!(matches!(&cmds[rect_i], DrawCmd::Rect { chamfer, fill, .. }
        if *chamfer == t.chamfer_large() && *fill == t.palette.panel));
}
