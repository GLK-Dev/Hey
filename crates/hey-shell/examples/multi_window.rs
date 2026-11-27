// Copyright (c) Vitaly Golik (mjojo)

//! MVU multi-window: a button opens another window with its own view function. Each extra window
//! can close itself; closing the first window quits.

use hey_core::Callback;
use hey_shell::{App, Command, WindowConfig, WindowKey};
use hey_style::Variant;
use hey_widgets::{Button, Label, View};

#[derive(Debug)]
enum Msg {
    Open,
    Close(WindowKey),
    Closed(WindowKey),
}

#[derive(Default)]
struct Model {
    next_key: WindowKey,
    open: Vec<WindowKey>,
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Open => {
            m.next_key += 1;
            let key = m.next_key;
            m.open.push(key);
            let cfg = WindowConfig { title: format!("Hey window {key}"), size: (420.0, 300.0), ..WindowConfig::default() };
            Command::open_window(key, cfg)
        }
        Msg::Close(key) => {
            m.open.retain(|k| *k != key);
            Command::close_window(key)
        }
        Msg::Closed(key) => {
            m.open.retain(|k| *k != key);
            Command::none()
        }
    }
}

fn main_view(m: &Model) -> Vec<View<Msg>> {
    let n = m.open.len();
    vec![
        View::leaf("count", n, move |ui, p| Label::add(ui, p, &format!("Extra windows: {n}"))),
        View::leaf("open", (), |ui, p| Button::add(ui, p, "Open window", Variant::Primary, Callback::new(|_| Msg::Open))),
    ]
}

fn extra_view(key: WindowKey, _: &Model) -> Vec<View<Msg>> {
    vec![
        View::leaf("title", key, move |ui, p| Label::add(ui, p, &format!("Window {key}"))),
        View::leaf("close", key, move |ui, p| Button::add(ui, p, "Close", Variant::Primary, Callback::new(move |_| Msg::Close(key)))),
    ]
}

fn main() {
    App::new(Model::default())
        .title("Hey multi-window")
        .update(update)
        .view(main_view)
        .window_view(extra_view)
        .on_window_closed(Msg::Closed)
        .run()
        .unwrap();
}
