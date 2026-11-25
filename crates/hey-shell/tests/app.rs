// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, MouseButton, Point, Size, UiEvent};
use hey_shell::{App, Command, ShellApp, TaskState};
use hey_style::Variant;
use hey_widgets::{Button, View};
use std::time::{Duration, Instant};

#[derive(Debug)]
enum Msg {
    Inc,
    Load,
    Loaded(Result<u32, String>),
}

#[derive(Default)]
struct Model {
    n: u32,
    task: TaskState<u32>,
}

fn view(m: &Model) -> Vec<View<Msg>> {
    let mut v = vec![View::leaf("inc", m.n, |ui, p| Button::add(ui, p, "Plus", Variant::Primary, Callback::new(|_| Msg::Inc)))];
    if !m.task.is_loading() {
        v.push(View::leaf("load", (), |ui, p| Button::add(ui, p, "Load", Variant::Primary, Callback::new(|_| Msg::Load))));
    }
    v
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Inc => {
            m.n += 1;
            Command::none()
        }
        Msg::Load => {
            m.task = TaskState::Loading;
            Command::perform(
                || {
                    std::thread::sleep(Duration::from_millis(30));
                    Ok::<u32, String>(42)
                },
                Msg::Loaded,
            )
        }
        Msg::Loaded(r) => {
            m.task = TaskState::from_result(r);
            Command::none()
        }
    }
}

fn click(rt: &mut impl ShellApp, p: Point) {
    rt.event(UiEvent::PointerDown { pos: p, button: MouseButton::Left });
    rt.event(UiEvent::PointerUp { pos: p, button: MouseButton::Left });
}

#[test]
fn mvu_and_background_task() {
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    let mut rt = App::new(Model::default()).update(update).view(view).font(font).build().unwrap();
    let vp = Size::new(400.0, 300.0);
    rt.frame(vp);

    // click on the first button (top-left corner of the content)
    let ids: Vec<_> = rt.ui.tree.children(rt.ui.root()).to_vec();
    let r = rt.ui.tree.rect(ids[0]);
    click(&mut rt, Point::new(r.x + 3.0, r.y + 3.0));
    assert_eq!(rt.model().n, 1);
    rt.frame(vp);

    // Load → Loading → the button disappears from view → Success
    let r = rt.ui.tree.rect(rt.ui.tree.children(rt.ui.root())[1]);
    click(&mut rt, Point::new(r.x + 3.0, r.y + 3.0));
    assert!(rt.model().task.is_loading());
    assert!(rt.pending());
    rt.frame(vp);
    assert_eq!(rt.ui.tree.children(rt.ui.root()).len(), 1);

    let t = Instant::now();
    while rt.pending() && t.elapsed() < Duration::from_secs(5) {
        rt.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    rt.tick();
    assert_eq!(rt.model().task, TaskState::Success(42));
    rt.frame(vp);
    assert_eq!(rt.ui.tree.children(rt.ui.root()).len(), 2);
}
