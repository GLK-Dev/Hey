// Copyright (c) Vitaly Golik (mjojo)

//! Widget showcase: tabs, buttons, input, selection, progress. State lives in `Model`.

use hey_core::{Callback, Insets};
use hey_layout::Style;
use hey_shell::{App, Command};
use hey_style::Variant;
use hey_widgets::{
    Button, Checkbox, Dropdown, Label, List, ProgressBar, Radio, RadioGroup, Slider, Tabs, TextArea, TextInput, Toggle, View,
};
use std::{cell::Cell, rc::Rc};

const TABS: [&str; 4] = ["Buttons", "Input", "Selection", "Progress"];
const FRUITS: [&str; 4] = ["Apple", "Pear", "Plum", "Cherry"];

#[derive(Debug)]
enum Msg {
    Tab(usize),
    Clicked(&'static str),
    Text(String),
    Notes(String),
    Check(bool),
    Toggle(bool),
    Radio(usize),
    Slide(f32),
    Pick(usize),
    Select(usize),
}

struct Model {
    tab: usize,
    last: String,
    text: String,
    notes: String,
    check: bool,
    toggle: bool,
    radio: usize,
    slider: f32,
    pick: usize,
    selected: usize,
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Tab(i) => m.tab = i,
        Msg::Clicked(s) => m.last = format!("Clicked: {s}"),
        Msg::Text(s) => m.text = s,
        Msg::Notes(s) => m.notes = s,
        Msg::Check(v) => m.check = v,
        Msg::Toggle(v) => m.toggle = v,
        Msg::Radio(i) => m.radio = i,
        Msg::Slide(v) => m.slider = v,
        Msg::Pick(i) => m.pick = i,
        Msg::Select(i) => m.selected = i,
    }
    Command::none()
}

fn label(key: &'static str, text: String) -> View<Msg> {
    View::leaf(key, text.clone(), move |ui, p| Label::add(ui, p, &text))
}

fn button(label: &'static str, variant: Variant) -> View<Msg> {
    View::leaf(label, variant as u8, move |ui, p| Button::add(ui, p, label, variant, Callback::new(move |_| Msg::Clicked(label))))
}

fn page(m: &Model) -> Vec<View<Msg>> {
    match m.tab {
        0 => vec![
            View::container(
                "variants",
                Style::row().gap(8.0),
                vec![button("Primary", Variant::Primary), button("Secondary", Variant::Secondary), button("Danger", Variant::Danger)],
            ),
            label("last", m.last.clone()),
        ],
        1 => vec![
            View::leaf("input", (), |ui, p| TextInput::add(ui, p, "", 360.0, Callback::new(Msg::Text))),
            label("echo", format!("Entered: {}", m.text)),
            View::leaf("area", (), |ui, p| TextArea::add(ui, p, "Multiline text", 360.0, 5, Callback::new(Msg::Notes))),
            label("len", format!("Characters in notes: {}", m.notes.chars().count())),
        ],
        2 => {
            let (c, t, r) = (m.check, m.toggle, m.radio);
            vec![
                View::leaf("check", c, move |ui, p| Checkbox::add(ui, p, "Checkbox", c, Callback::new(Msg::Check))),
                View::leaf("toggle", t, move |ui, p| Toggle::add(ui, p, "Toggle", t, Callback::new(Msg::Toggle))),
                View::leaf("radios", r, move |ui, p| {
                    let col = ui.add_container(p, Style::column().gap(4.0));
                    let group: RadioGroup = Rc::new(Cell::new(Some(r)));
                    for (i, name) in ["One", "Two", "Three"].iter().enumerate() {
                        Radio::add(ui, col, name, i, group.clone(), Callback::new(Msg::Radio));
                    }
                    col
                }),
                View::leaf("dd", m.pick, {
                    let pick = m.pick;
                    move |ui, p| Dropdown::add(ui, p, FRUITS.iter().map(|s| s.to_string()).collect(), pick, 200.0, Callback::new(Msg::Pick))
                }),
                View::leaf("list", (), |ui, p| {
                    List::add(ui, p, FRUITS.iter().map(|s| s.to_string()).collect(), 200.0, 4, Callback::new(Msg::Select))
                }),
                label("sel", format!("List: {}, dropdown: {}", FRUITS[m.selected], FRUITS[m.pick])),
            ]
        }
        _ => {
            let v = m.slider;
            vec![
                View::leaf("slider", (), move |ui, p| Slider::add(ui, p, 0.0, 100.0, v, 300.0, Callback::new(Msg::Slide))),
                View::leaf("bar", v.to_bits(), move |ui, p| ProgressBar::add(ui, p, v / 100.0, 300.0)),
                label("val", format!("Value: {v:.0}%")),
            ]
        }
    }
}

fn view(m: &Model) -> Vec<View<Msg>> {
    let tab = m.tab;
    vec![View::container(
        "root",
        Style::column().padding(Insets::all(24.0)).gap(12.0),
        vec![
            View::leaf("title", (), |ui, p| Label::add(ui, p, "Hey widget gallery")),
            View::leaf("tabs", tab, move |ui, p| Tabs::add(ui, p, &TABS, tab, Callback::new(Msg::Tab))),
            View::container("page", Style::column().padding(Insets::all(16.0)).gap(10.0), page(m)),
        ],
    )]
}

fn main() {
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    let model = Model {
        tab: 0,
        last: "Press a button".into(),
        text: String::new(),
        notes: String::new(),
        check: false,
        toggle: false,
        radio: 0,
        slider: 40.0,
        pick: 0,
        selected: 0,
    };
    if let Err(e) = App::new(model).update(update).view(view).font(font).title("Hey — gallery").run() {
        eprintln!("error: {e}");
    }
}
