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

#[derive(Debug)]
enum WMsg {
    Open(u32),
    Close(u32),
    Closed(u32),
}

#[derive(Default)]
struct WModel {
    open: Vec<u32>,
    closed_by_user: Vec<u32>,
}

fn w_update(m: &mut WModel, msg: WMsg) -> Command<WMsg> {
    match msg {
        WMsg::Open(k) => {
            m.open.push(k);
            Command::open_window(k, hey_shell::WindowConfig::default())
        }
        WMsg::Close(k) => {
            m.open.retain(|o| *o != k);
            Command::close_window(k)
        }
        WMsg::Closed(k) => {
            m.open.retain(|o| *o != k);
            m.closed_by_user.push(k);
            Command::none()
        }
    }
}

fn w_main(_: &WModel) -> Vec<View<WMsg>> {
    vec![View::leaf("open", (), |ui, p| Button::add(ui, p, "Open", Variant::Primary, Callback::new(|_| WMsg::Open(1))))]
}

fn w_extra(key: u32, _: &WModel) -> Vec<View<WMsg>> {
    vec![
        View::leaf("close", key, move |ui, p| Button::add(ui, p, "Close", Variant::Primary, Callback::new(move |_| WMsg::Close(key)))),
        View::leaf("other", (), |ui, p| Button::add(ui, p, "Other", Variant::Primary, Callback::new(|_| WMsg::Open(2)))),
    ]
}

#[test]
fn extra_windows_have_own_views_and_close() {
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    let mut rt = App::new(WModel::default())
        .update(w_update)
        .view(w_main)
        .window_view(w_extra)
        .on_window_closed(WMsg::Closed)
        .font(font)
        .build()
        .unwrap();
    let vp = Size::new(400.0, 300.0);
    rt.frame(vp);
    assert_eq!(rt.window_count(), 1);

    // Main window: click "Open" -> a second window is requested.
    let r = rt.ui.tree.rect(rt.ui.tree.children(rt.ui.root())[0]);
    click(&mut rt, Point::new(r.x + 3.0, r.y + 3.0));
    assert_eq!(rt.window_count(), 2);
    let new = rt.take_new_windows();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].0, 1);

    // The shell focuses the new window before drawing it: it has its own two-button view.
    rt.focus_window(1);
    rt.frame(vp);
    assert_eq!(rt.ui.tree.children(rt.ui.root()).len(), 2);
    rt.focus_window(hey_shell::MAIN_WINDOW);
    assert_eq!(rt.ui.tree.children(rt.ui.root()).len(), 1);

    // Closing from inside the extra window removes it and reports it to the shell.
    rt.focus_window(1);
    let r = rt.ui.tree.rect(rt.ui.tree.children(rt.ui.root())[0]);
    click(&mut rt, Point::new(r.x + 3.0, r.y + 3.0));
    assert_eq!(rt.window_count(), 1);
    assert_eq!(rt.take_closed_windows(), vec![1]);
    assert!(rt.model().open.is_empty());

    // A window closed by the user produces `on_window_closed`.
    rt.focus_window(hey_shell::MAIN_WINDOW);
    rt.frame(vp);
    let r = rt.ui.tree.rect(rt.ui.tree.children(rt.ui.root())[0]);
    click(&mut rt, Point::new(r.x + 3.0, r.y + 3.0));
    assert_eq!(rt.window_count(), 2);
    rt.window_closed(1);
    let t = Instant::now();
    while rt.model().closed_by_user.is_empty() && t.elapsed() < Duration::from_secs(5) {
        rt.tick();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(rt.window_count(), 1);
    assert_eq!(rt.model().closed_by_user, vec![1]);
}

#[derive(Debug)]
enum DMsg {
    Select(u32),
    Float(u32),
    Closed(u32),
}

struct DModel {
    dock: hey_widgets::Dock,
}

fn d_update(m: &mut DModel, msg: DMsg) -> Command<DMsg> {
    match msg {
        DMsg::Select(p) => {
            m.dock.select(p);
            Command::none()
        }
        DMsg::Float(p) => {
            m.dock.float(p, p);
            Command::open_window(p, hey_shell::WindowConfig::default())
        }
        DMsg::Closed(w) => {
            m.dock.dock_back(w);
            Command::none()
        }
    }
}

fn d_content(p: u32) -> Vec<View<DMsg>> {
    vec![View::leaf("float", p, move |ui, parent| {
        Button::add(ui, parent, "Float", Variant::Primary, Callback::new(move |_| DMsg::Float(p)))
    })]
}

fn d_render(root: Option<&hey_widgets::DockNode>) -> Vec<View<DMsg>> {
    match root {
        Some(n) => vec![hey_widgets::dock_view(n, &|p| format!("Panel {p}"), &d_content, &Callback::new(DMsg::Select))],
        None => Vec::new(),
    }
}

#[test]
fn docking_floats_a_panel_into_its_own_window() {
    use hey_widgets::{Dock, DockNode};
    let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/fonts/onest/Onest[wght].ttf");
    let model = DModel { dock: Dock::new(DockNode::group([1, 2])) };
    let mut rt = App::new(model)
        .update(d_update)
        .view(|m: &DModel| d_render(m.dock.main()))
        .window_view(|w, m: &DModel| d_render(m.dock.floating(w)))
        .on_window_closed(DMsg::Closed)
        .font(font)
        .build()
        .unwrap();
    let vp = Size::new(500.0, 400.0);
    rt.frame(vp);
    assert_eq!(rt.ui.tree.children(rt.ui.root()).len(), 1);

    // Find the active panel's "Float" button: the deepest leaf of the tree.
    fn deepest(rt: &impl Fn(hey_core::NodeId) -> Vec<hey_core::NodeId>, id: hey_core::NodeId) -> hey_core::NodeId {
        let k = rt(id);
        if k.is_empty() {
            id
        } else {
            deepest(rt, *k.last().unwrap())
        }
    }
    let kids = |id| rt.ui.tree.children(id).to_vec();
    let leaf = deepest(&kids, rt.ui.root());
    let r = rt.ui.tree.rect(leaf);
    click(&mut rt, Point::new(r.x + 3.0, r.y + 3.0));
    assert_eq!(rt.model().dock.main().unwrap().panels(), vec![2]);
    assert_eq!(rt.model().dock.floating(1).unwrap().panels(), vec![1]);
    assert_eq!(rt.window_count(), 2);

    // The floating window renders its own root; closing it docks the panel back.
    rt.focus_window(1);
    rt.frame(vp);
    assert_eq!(rt.ui.tree.children(rt.ui.root()).len(), 1);
    rt.window_closed(1);
    let t = Instant::now();
    while rt.model().dock.floating(1).is_some() && t.elapsed() < Duration::from_secs(5) {
        rt.tick();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(rt.model().dock.main().unwrap().panels().len(), 2);
}
