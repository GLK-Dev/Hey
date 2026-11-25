// Copyright (c) Vitaly Golik (mjojo)

//! Список задач: ввод текста, флажки, фильтр, фоновая загрузка/сохранение (`Command::perform`).

use hey_core::{Callback, Insets};
use hey_layout::Style;
use hey_shell::{App, Command, TaskState};
use hey_style::Variant;
use hey_widgets::{Button, Checkbox, Label, TextInput, View};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
struct Item {
    id: u32,
    text: String,
    done: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    All,
    Active,
    Done,
}

#[derive(Debug)]
enum Msg {
    Input(String),
    Add,
    Toggle(u32, bool),
    Remove(u32),
    SetFilter(Filter),
    Loaded(Result<Vec<Item>, String>),
    Saved(Result<(), String>),
}

struct Model {
    items: Vec<Item>,
    input: String,
    /// Меняется после добавления: пересоздаёт поле ввода пустым.
    input_gen: u32,
    next_id: u32,
    filter: Filter,
    io: TaskState<()>,
}

fn path() -> PathBuf {
    std::env::temp_dir().join("hey_todo.txt")
}

fn load() -> Result<Vec<Item>, String> {
    let s = match std::fs::read_to_string(path()) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    Ok(s.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let (d, t) = l.split_once('|')?;
            Some(Item { id: i as u32, text: t.to_string(), done: d == "1" })
        })
        .collect())
}

fn save(items: Vec<Item>) -> Result<(), String> {
    let s: String = items.iter().map(|i| format!("{}|{}\n", if i.done { 1 } else { 0 }, i.text)).collect();
    std::fs::write(path(), s).map_err(|e| e.to_string())
}

fn persist(m: &mut Model) -> Command<Msg> {
    m.io = TaskState::Loading;
    let items = m.items.clone();
    Command::perform(move || save(items), Msg::Saved)
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Input(s) => {
            m.input = s;
            Command::none()
        }
        Msg::Add => {
            let t = m.input.trim().to_string();
            if t.is_empty() {
                return Command::none();
            }
            m.items.push(Item { id: m.next_id, text: t, done: false });
            m.next_id += 1;
            m.input.clear();
            m.input_gen += 1;
            persist(m)
        }
        Msg::Toggle(id, v) => {
            if let Some(i) = m.items.iter_mut().find(|i| i.id == id) {
                i.done = v;
            }
            persist(m)
        }
        Msg::Remove(id) => {
            m.items.retain(|i| i.id != id);
            persist(m)
        }
        Msg::SetFilter(f) => {
            m.filter = f;
            Command::none()
        }
        Msg::Loaded(r) => {
            match r {
                Ok(items) => {
                    m.next_id = items.iter().map(|i| i.id + 1).max().unwrap_or(0);
                    m.items = items;
                    m.io = TaskState::Idle;
                }
                Err(e) => m.io = TaskState::Error(e),
            }
            Command::none()
        }
        Msg::Saved(r) => {
            m.io = TaskState::from_result(r);
            Command::none()
        }
    }
}

fn filter_button(m: &Model, key: &'static str, label: &'static str, f: Filter) -> View<Msg> {
    let variant = if m.filter == f { Variant::Primary } else { Variant::Secondary };
    View::leaf(key, (label, variant as u8), move |ui, p| Button::add(ui, p, label, variant, Callback::new(move |_| Msg::SetFilter(f))))
}

fn row(i: &Item) -> View<Msg> {
    let (id, done, text) = (i.id, i.done, i.text.clone());
    let t2 = text.clone();
    View::container(
        id as u64,
        Style::row().gap(8.0),
        vec![
            View::leaf("check", (done, t2), move |ui, p| Checkbox::add(ui, p, &text, done, Callback::new(move |v| Msg::Toggle(id, v)))),
            View::leaf("del", (), move |ui, p| Button::add(ui, p, "Удалить", Variant::Secondary, Callback::new(move |_| Msg::Remove(id)))),
        ],
    )
}

fn view(m: &Model) -> Vec<View<Msg>> {
    let status = match &m.io {
        TaskState::Loading => "Сохранение…".to_string(),
        TaskState::Error(e) => format!("Ошибка: {e}"),
        _ => format!("Задач: {}, выполнено: {}", m.items.len(), m.items.iter().filter(|i| i.done).count()),
    };
    let gen = m.input_gen;
    let mut children = vec![
        View::leaf("title", (), |ui, p| Label::add(ui, p, "Список задач")),
        View::container(
            "add",
            Style::row().gap(8.0),
            vec![
                View::leaf("input", gen, |ui, p| TextInput::add(ui, p, "", 320.0, Callback::new(Msg::Input))),
                View::leaf("add", (), |ui, p| Button::add(ui, p, "Добавить", Variant::Primary, Callback::new(|_| Msg::Add))),
            ],
        ),
        View::container(
            "filters",
            Style::row().gap(8.0),
            vec![
                filter_button(m, "all", "Все", Filter::All),
                filter_button(m, "active", "Активные", Filter::Active),
                filter_button(m, "done", "Готовые", Filter::Done),
            ],
        ),
    ];
    children.extend(
        m.items
            .iter()
            .filter(|i| match m.filter {
                Filter::All => true,
                Filter::Active => !i.done,
                Filter::Done => i.done,
            })
            .map(row),
    );
    children.push(View::leaf("status", status.clone(), move |ui, p| Label::add(ui, p, &status)));
    vec![View::container("root", Style::column().padding(Insets::all(24.0)).gap(12.0), children)]
}

fn main() {
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    let model = Model { items: vec![], input: String::new(), input_gen: 0, next_id: 0, filter: Filter::All, io: TaskState::Loading };
    let app = App::new(model).update(update).view(view).font(font).title("Hey — todo").init(Command::perform(load, Msg::Loaded));
    if let Err(e) = app.run() {
        eprintln!("ошибка: {e}");
    }
}
