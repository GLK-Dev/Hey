// Copyright (c) Vitaly Golik (mjojo)

//! Счётчик: минимальный MVU-пример (`Model` → `view` → `Msg` → `update`).

use hey_core::{Callback, Insets};
use hey_layout::Style;
use hey_shell::{App, Command};
use hey_style::{Theme, Variant};
use hey_widgets::{Button, Label, View};

#[derive(Clone, Copy, Debug)]
enum Msg {
    Inc,
    Dec,
    Reset,
}

#[derive(Default)]
struct Model {
    n: i64,
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Inc => m.n += 1,
        Msg::Dec => m.n -= 1,
        Msg::Reset => m.n = 0,
    }
    Command::none()
}

fn button(key: &'static str, label: &'static str, variant: Variant, msg: Msg) -> View<Msg> {
    View::leaf(key, (label, variant as u8), move |ui, p| Button::add(ui, p, label, variant, Callback::new(move |_| msg)))
}

fn view(m: &Model) -> Vec<View<Msg>> {
    let n = m.n;
    vec![View::container(
        "root",
        Style::column().padding(Insets::all(24.0)).gap(16.0),
        vec![
            View::leaf("title", (), |ui, p| Label::add(ui, p, "Счётчик")),
            View::leaf("value", n, move |ui, p| Label::add(ui, p, &format!("Значение: {n}"))),
            View::container(
                "buttons",
                Style::row().gap(8.0),
                vec![
                    button("dec", "−", Variant::Secondary, Msg::Dec),
                    button("inc", "+", Variant::Primary, Msg::Inc),
                    button("reset", "Сброс", Variant::Secondary, Msg::Reset),
                ],
            ),
        ],
    )]
}

fn main() {
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    if let Err(e) = App::new(Model::default()).update(update).view(view).theme(Theme::dark()).font(font).title("Hey — counter").run() {
        eprintln!("ошибка: {e}");
    }
}
