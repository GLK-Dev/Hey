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

#[test]
fn auto_theme_follows_system_appearance() {
    use hey_style::{Appearance, Theme};
    let build = |auto| App::new(Model::default()).update(update).view(view).auto_theme(auto).build().unwrap();

    let mut fixed = build(false);
    assert!(!fixed.appearance_changed(Appearance { dark: false, ..Appearance::default() }));
    assert_eq!(fixed.ui.env.theme.id, Theme::dark().id);
    // A fixed theme still follows the system text scale.
    assert!(fixed.appearance_changed(Appearance { text_scale: 150, ..Appearance::default() }));
    assert_eq!(fixed.ui.env.theme.text_scale, 1.5);
    assert_eq!(fixed.ui.env.font_px, 21.0);
    assert!(!fixed.appearance_changed(Appearance { text_scale: 150, ..Appearance::default() }));

    let mut auto = build(true);
    assert!(auto.appearance_changed(Appearance { dark: false, high_contrast: false, reduce_motion: true, ..Appearance::default() }));
    assert_eq!(auto.ui.env.theme.palette, Theme::light().palette);
    assert_eq!(auto.ui.env.theme.base_ms, 0.0);
    assert!(auto.appearance_changed(Appearance { dark: true, high_contrast: true, reduce_motion: false, ..Appearance::default() }));
    assert_eq!(auto.ui.env.theme.id, "high-contrast-dark");
    auto.draw(hey_draw::Viewport { width: 200, height: 100, scale: 1.0 }, &mut hey_draw::DisplayList::default());
}

#[test]
fn time_travel_rewinds_100_messages_without_rerunning_commands() {
    use hey_shell::Seek;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Clone, Debug, PartialEq)]
    struct M(u32);
    #[derive(Debug)]
    enum Ms {
        Inc,
        Noop,
    }
    let runs = Rc::new(Cell::new(0));
    let r = runs.clone();
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    let mut rt = App::new(M(0))
        .time_travel()
        .update(move |m: &mut M, msg: Ms| {
            r.set(r.get() + 1);
            match msg {
                Ms::Inc => {
                    m.0 += 1;
                    // A command that must not run again on rewind.
                    Command::perform(|| (), |_| Ms::Noop)
                }
                Ms::Noop => Command::none(),
            }
        })
        .view(|m: &M| vec![View::leaf("inc", m.0, |ui, p| Button::add(ui, p, "Plus", Variant::Primary, Callback::new(|_| Ms::Inc)))])
        .font(font)
        .build()
        .unwrap();
    let vp = Size::new(400.0, 300.0);
    rt.frame(vp);
    let r0 = rt.ui.tree.rect(rt.ui.tree.children(rt.ui.root())[0]);
    let p = Point::new(r0.x + 3.0, r0.y + 3.0);
    for _ in 0..120 {
        click(&mut rt, p);
        rt.frame(vp);
    }
    // Let background results arrive, then settle.
    let t = Instant::now();
    while rt.pending() && t.elapsed() < Duration::from_secs(5) {
        rt.tick();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(rt.model().0, 120);
    let (len, runs_before) = (rt.history_len(), runs.get());
    assert!(len >= 120);
    for _ in 0..100 {
        assert!(rt.time_travel_seek(Seek::Back));
    }
    assert_eq!(rt.history_cursor(), len - 100);
    assert!(rt.model().0 < 120);
    assert_eq!(runs.get(), runs_before, "update must not run again");
    assert_eq!(rt.history_len(), len, "stepping adds no history");
    assert!(rt.ui.note().is_some());
    let _ = rt.frame(vp);
    // Input is ignored while rewound.
    let shown = rt.model().0;
    click(&mut rt, p);
    assert_eq!(rt.model().0, shown);
    assert_eq!(runs.get(), runs_before);
    assert!(rt.time_travel_seek(Seek::Live));
    assert_eq!(rt.model().0, 120);
    assert!(rt.ui.note().is_none());
    assert!(rt.time_travel_seek(Seek::Start));
    assert_eq!(rt.model().0, 0);
}
