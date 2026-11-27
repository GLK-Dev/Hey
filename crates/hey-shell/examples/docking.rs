// Copyright (c) Vitaly Golik (mjojo)

//! Dockable panels: tabbed groups in a split layout. "Float" tears a panel into its own window;
//! closing that window docks it back. "Split" moves a panel below the first one.

use hey_core::Callback;
use hey_layout::Axis;
use hey_shell::{App, Command, WindowConfig};
use hey_style::Variant;
use hey_widgets::{dock_view, Button, Dock, DockNode, DropZone, Label, View};

#[derive(Debug)]
enum Msg {
    Select(u32),
    Float(u32),
    Split(u32),
    Closed(u32),
}

struct Model {
    dock: Dock,
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Select(p) => {
            m.dock.select(p);
            Command::none()
        }
        Msg::Float(p) => {
            m.dock.float(p, p);
            Command::open_window(p, WindowConfig { title: format!("Panel {p}"), size: (360.0, 260.0), ..WindowConfig::default() })
        }
        Msg::Split(p) => {
            if let Some(first) = m.dock.panels().into_iter().find(|x| *x != p) {
                m.dock.move_panel(p, first, DropZone::Bottom);
            }
            Command::none()
        }
        Msg::Closed(w) => {
            m.dock.dock_back(w);
            Command::none()
        }
    }
}

fn content(p: u32) -> Vec<View<Msg>> {
    vec![
        View::leaf("label", p, move |ui, parent| Label::add(ui, parent, &format!("Content of panel {p}"))),
        View::leaf("float", p, move |ui, parent| Button::add(ui, parent, "Float", Variant::Primary, Callback::new(move |_| Msg::Float(p)))),
        View::leaf("split", p, move |ui, parent| Button::add(ui, parent, "Split", Variant::Primary, Callback::new(move |_| Msg::Split(p)))),
    ]
}

fn render(root: Option<&DockNode>) -> Vec<View<Msg>> {
    root.map(|n| vec![dock_view(n, &|p| format!("Panel {p}"), &content, &Callback::new(Msg::Select))]).unwrap_or_default()
}

fn main() {
    let layout = DockNode::split(Axis::Row, 0.35, DockNode::group([1, 2]), DockNode::group([3, 4]));
    App::new(Model { dock: Dock::new(layout) })
        .title("Hey docking")
        .update(update)
        .view(|m: &Model| render(m.dock.main()))
        .window_view(|w, m: &Model| render(m.dock.floating(w)))
        .on_window_closed(Msg::Closed)
        .run()
        .unwrap();
}
