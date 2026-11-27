// Copyright (c) Vitaly Golik (mjojo)

//! Public application API (MVU): `App::new(model).update(f).view(f).theme(t).run()`.
//!
//! Loop: event → `Ui` puts `Msg` into `MsgSink` → `update(&mut model, msg) -> Command` →
//! `view(&model)` → `Ui::reconcile`. Background tasks (`Command::perform`) run on threads,
//! results come back via `mpsc` and are polled in `tick()`.

use crate::{Error, ShellApp, WindowConfig};
use hey_core::{MsgSink, Size, UiEvent};
use hey_draw::{DisplayList, Viewport};
use hey_style::Theme;
use hey_text::{FontId, Fonts};
use hey_widgets::{Ui, View};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

/// State of a background operation for displaying idle/loading/success/error.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TaskState<T> {
    #[default]
    Idle,
    Loading,
    Success(T),
    Error(String),
}

impl<T> TaskState<T> {
    pub fn from_result<E: std::fmt::Display>(r: Result<T, E>) -> Self {
        match r {
            Ok(v) => TaskState::Success(v),
            Err(e) => TaskState::Error(e.to_string()),
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, TaskState::Loading)
    }
}

/// Default font (Onest, SIL OFL 1.1).
const DEFAULT_FONT: &[u8] = hey_text::DEFAULT_FONT;

type Job<Msg> = Box<dyn FnOnce() -> Msg + Send>;

/// Side effect returned from `update`.
pub enum Command<Msg> {
    None,
    /// Send a message right away (after the current pass).
    Msg(Msg),
    /// Run work on a background thread; the result is turned into a `Msg`.
    Task(Job<Msg>),
    Batch(Vec<Command<Msg>>),
}

impl<Msg: Send + 'static> Command<Msg> {
    pub fn none() -> Self {
        Command::None
    }

    pub fn msg(m: Msg) -> Self {
        Command::Msg(m)
    }

    /// `work` runs on a separate thread, `map` turns the result into a `Msg`.
    pub fn perform<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, map: impl FnOnce(T) -> Msg + Send + 'static) -> Self {
        Command::Task(Box::new(move || map(work())))
    }

    pub fn batch(cmds: impl IntoIterator<Item = Command<Msg>>) -> Self {
        Command::Batch(cmds.into_iter().collect())
    }
}

type UpdateFn<M, Msg> = Box<dyn FnMut(&mut M, Msg) -> Command<Msg>>;
type ViewFn<M, Msg> = Box<dyn Fn(&M) -> Vec<View<Msg>>>;

/// Time-travel scrubber moves (dev hotkeys Ctrl+Shift+Left / Right / Home / End).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    /// One message back.
    Back,
    /// One message forward.
    Forward,
    /// The initial model.
    Start,
    /// The live model (leaves the rewound state).
    Live,
}

/// Most snapshots kept; the oldest are dropped first.
const HISTORY_CAP: usize = 1000;

type CloneFn<M> = Box<dyn Fn(&M) -> M>;
type LabelFn<Msg> = Box<dyn Fn(&Msg) -> String>;

/// Model snapshots taken after every message. Rewinding swaps in a snapshot, so no `update` and no
/// command runs again: the log is replayed as data, not re-executed.
struct TimeTravel<M, Msg> {
    /// `(message that led here, model after it)`; entry 0 is the initial model.
    history: Vec<(String, M)>,
    /// Index of the shown entry; `history.len() - 1` is live.
    cursor: usize,
    clone: CloneFn<M>,
    label: LabelFn<Msg>,
}

impl<M, Msg> TimeTravel<M, Msg> {
    fn rewound(&self) -> bool {
        self.cursor + 1 < self.history.len()
    }

    fn note(&self) -> Option<String> {
        self.rewound().then(|| {
            let (label, _) = &self.history[self.cursor];
            format!("time travel {}/{}: {}", self.cursor, self.history.len() - 1, label)
        })
    }
}

/// Application builder.
pub struct App<M, Msg: Send + 'static> {
    model: M,
    update: Option<UpdateFn<M, Msg>>,
    view: Option<ViewFn<M, Msg>>,
    theme: Theme,
    auto_theme: bool,
    theme_file: Option<(PathBuf, String)>,
    fonts: Vec<PathBuf>,
    window: WindowConfig,
    init: Command<Msg>,
    time_travel: Option<TtHooks<M, Msg>>,
}

type TtHooks<M, Msg> = (CloneFn<M>, LabelFn<Msg>);

impl<M: 'static, Msg: Send + 'static> App<M, Msg> {
    pub fn new(model: M) -> Self {
        App {
            model,
            update: None,
            view: None,
            theme: Theme::dark(),
            auto_theme: false,
            theme_file: None,
            fonts: Vec::new(),
            window: WindowConfig::default(),
            init: Command::None,
            time_travel: None,
        }
    }

    /// Dev feature: keep a model snapshot after every message (the last 1000) so Ctrl+Shift+Left /
    /// Right / Home / End step through history and re-render any frame. Stepping back restores a
    /// snapshot; `update` and commands do not run again. While rewound, new messages are dropped.
    pub fn time_travel(mut self) -> Self
    where
        M: Clone,
        Msg: std::fmt::Debug,
    {
        self.time_travel = Some((
            Box::new(M::clone),
            Box::new(|m: &Msg| {
                let s = format!("{m:?}");
                if s.chars().count() > 60 {
                    format!("{}...", s.chars().take(57).collect::<String>())
                } else {
                    s
                }
            }),
        ));
        self
    }

    pub fn update(mut self, f: impl FnMut(&mut M, Msg) -> Command<Msg> + 'static) -> Self {
        self.update = Some(Box::new(f));
        self
    }

    pub fn view(mut self, f: impl Fn(&M) -> Vec<View<Msg>> + 'static) -> Self {
        self.view = Some(Box::new(f));
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Follow the system appearance (dark/light, high contrast, reduced motion) instead of the fixed theme.
    pub fn auto_theme(mut self, on: bool) -> Self {
        self.auto_theme = on;
        self
    }

    /// Load theme `id` from a `themes.json`-format file and reload it whenever the file changes (hot reload).
    /// A failed initial load keeps the `theme`; a failed reload keeps the current theme.
    /// While watching, the loop wakes every ~16 ms to poll the file, so use it during development.
    pub fn watch_theme(mut self, path: impl Into<PathBuf>, id: impl Into<String>) -> Self {
        self.theme_file = Some((path.into(), id.into()));
        self
    }

    /// Add a font file; the order defines the fallback chain.
    pub fn font(mut self, path: impl Into<PathBuf>) -> Self {
        self.fonts.push(path.into());
        self
    }

    pub fn window(mut self, cfg: WindowConfig) -> Self {
        self.window = cfg;
        self
    }

    /// Wait for vertical sync on the GPU backend (default `true`; `HEY_VSYNC=0|1` overrides).
    pub fn vsync(mut self, on: bool) -> Self {
        self.window.vsync = on;
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into();
        self
    }

    /// Command run at startup (e.g. the initial load).
    pub fn init(mut self, cmd: Command<Msg>) -> Self {
        self.init = cmd;
        self
    }

    /// Build the runtime without a window (for tests and embedding).
    pub fn build(self) -> Result<Runtime<M, Msg>, Error> {
        let update = self.update.ok_or(Error::MissingCallback("update"))?;
        let view = self.view.ok_or(Error::MissingCallback("view"))?;
        let mut fonts = Fonts::new();
        let mut chain: Vec<FontId> = Vec::new();
        for p in &self.fonts {
            chain.push(fonts.add_file(p).map_err(|e| Error::Font { path: Some(p.clone()), message: format!("{e:?}") })?);
        }
        if chain.is_empty() {
            // Default font is Onest (OFL, Cyrillic), embedded into the binary.
            chain.push(fonts.add_bytes(DEFAULT_FONT).map_err(|e| Error::Font { path: None, message: format!("{e:?}") })?);
        }
        let sink = MsgSink::new();
        let watch = self.theme_file.map(|(path, id)| ThemeWatch::new(path, id));
        let theme = watch.as_ref().and_then(|w| w.load().ok()).unwrap_or(self.theme);
        let ui = Ui::new(theme, Arc::new(fonts), chain, sink.clone());
        let (tx, rx) = mpsc::channel();
        let tt = self.time_travel.map(|(clone, label)| TimeTravel {
            history: vec![("init".to_string(), clone(&self.model))],
            cursor: 0,
            clone,
            label,
        });
        let mut rt = Runtime {
            tt,
            model: self.model,
            update,
            view,
            ui,
            sink,
            tx,
            rx,
            in_flight: Arc::new(AtomicUsize::new(0)),
            size: Size::new(0.0, 0.0),
            clear: self.window.clear,
            auto_theme: self.auto_theme,
            watch,
            last_tick: std::time::Instant::now(),
            dirty: true,
        };
        rt.run_command(self.init);
        rt.rebuild();
        Ok(rt)
    }

    pub fn run(self) -> Result<(), Error> {
        let cfg =
            WindowConfig { title: self.window.title.clone(), size: self.window.size, clear: self.window.clear, vsync: self.window.vsync };
        let rt = self.build()?;
        crate::run(cfg, rt)
    }
}

/// Polls a theme file's modification time.
struct ThemeWatch {
    path: PathBuf,
    id: String,
    seen: Option<std::time::SystemTime>,
    checked: std::time::Instant,
}

impl ThemeWatch {
    const INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

    fn new(path: PathBuf, id: String) -> Self {
        let seen = Self::mtime(&path);
        ThemeWatch { path, id, seen, checked: std::time::Instant::now() }
    }

    fn mtime(path: &std::path::Path) -> Option<std::time::SystemTime> {
        std::fs::metadata(path).and_then(|m| m.modified()).ok()
    }

    fn load(&self) -> Result<Theme, hey_style::ThemeError> {
        Theme::from_file(&self.path, &self.id)
    }

    /// A new theme if the file changed since the last check and parses.
    fn changed(&mut self) -> Option<Theme> {
        if self.checked.elapsed() < Self::INTERVAL {
            return None;
        }
        self.checked = std::time::Instant::now();
        let m = Self::mtime(&self.path)?;
        if self.seen == Some(m) {
            return None;
        }
        self.seen = Some(m);
        self.load().ok()
    }
}

/// App runtime: implements `ShellApp`.
pub struct Runtime<M, Msg: Send + 'static> {
    tt: Option<TimeTravel<M, Msg>>,
    model: M,
    update: UpdateFn<M, Msg>,
    view: ViewFn<M, Msg>,
    pub ui: Ui<Msg>,
    sink: MsgSink<Msg>,
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,
    in_flight: Arc<AtomicUsize>,
    size: Size,
    #[allow(dead_code)]
    clear: hey_core::Color,
    auto_theme: bool,
    watch: Option<ThemeWatch>,
    last_tick: std::time::Instant,
    dirty: bool,
}

impl<M: 'static, Msg: Send + 'static> Runtime<M, Msg> {
    pub fn model(&self) -> &M {
        &self.model
    }

    fn rebuild(&mut self) {
        let views = (self.view)(&self.model);
        let root = self.ui.root();
        self.ui.reconcile(root, views);
        self.dirty = false;
    }

    fn run_command(&mut self, cmd: Command<Msg>) {
        match cmd {
            Command::None => {}
            Command::Msg(m) => self.sink.send(m),
            Command::Batch(v) => v.into_iter().for_each(|c| self.run_command(c)),
            Command::Task(job) => {
                let tx = self.tx.clone();
                let n = self.in_flight.clone();
                n.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    let msg = job();
                    let _ = tx.send(msg);
                    n.fetch_sub(1, Ordering::SeqCst);
                });
            }
        }
    }

    /// Run all pending messages through `update`; `true` if anything was processed.
    pub fn pump(&mut self) -> bool {
        let mut any = false;
        loop {
            let msgs = self.sink.drain();
            if msgs.is_empty() {
                break;
            }
            any = true;
            for m in msgs {
                if self.tt.as_ref().is_some_and(TimeTravel::rewound) {
                    continue; // history is read-only while rewound
                }
                let label = self.tt.as_ref().map(|t| (t.label)(&m));
                let cmd = (self.update)(&mut self.model, m);
                if let (Some(t), Some(label)) = (&mut self.tt, label) {
                    t.history.push((label, (t.clone)(&self.model)));
                    if t.history.len() > HISTORY_CAP {
                        t.history.remove(0);
                    }
                    t.cursor = t.history.len() - 1;
                }
                self.run_command(cmd);
            }
        }
        if any {
            self.dirty = true;
        }
        any
    }

    /// Number of recorded messages (0 without `App::time_travel`).
    pub fn history_len(&self) -> usize {
        self.tt.as_ref().map_or(0, |t| t.history.len() - 1)
    }

    /// Shown position: `0` is the initial model, `history_len()` is live.
    pub fn history_cursor(&self) -> usize {
        self.tt.as_ref().map_or(0, |t| t.cursor)
    }

    /// Jumps to snapshot `index` (clamped) and marks the view for rebuild. No `update` runs and no
    /// command is started. `false` without time travel or when nothing changed.
    pub fn seek(&mut self, index: usize) -> bool {
        let Some(t) = &mut self.tt else { return false };
        let index = index.min(t.history.len() - 1);
        if index == t.cursor {
            return false;
        }
        t.cursor = index;
        self.model = (t.clone)(&t.history[index].1);
        let note = t.note();
        self.ui.set_note(note);
        self.dirty = true;
        true
    }

    /// Hotkey-style move; see [`Seek`].
    pub fn time_travel_seek(&mut self, to: Seek) -> bool {
        let Some(t) = &self.tt else { return false };
        let target = match to {
            Seek::Back => t.cursor.saturating_sub(1),
            Seek::Forward => t.cursor + 1,
            Seek::Start => 0,
            Seek::Live => usize::MAX,
        };
        self.seek(target)
    }

    /// Build a frame for the `size` area (physical pixels).
    pub fn frame(&mut self, size: Size) -> DisplayList {
        self.size = size;
        if self.dirty {
            self.rebuild();
        }
        self.ui.paint(size)
    }

    /// Collect the results of background tasks.
    pub fn poll(&mut self) -> bool {
        let mut got = false;
        while let Ok(m) = self.rx.try_recv() {
            self.sink.send(m);
            got = true;
        }
        got | self.pump()
    }
}

impl<M: 'static, Msg: Send + 'static> ShellApp for Runtime<M, Msg> {
    fn draw(&mut self, viewport: Viewport, list: &mut DisplayList) {
        self.ui.set_scale(viewport.scale);
        let was = self.ui.animating();
        *list = self.frame(Size::new(viewport.width as f32, viewport.height as f32));
        if !was {
            // A crossfade that starts in this frame is timed from now.
            self.last_tick = std::time::Instant::now();
        }
    }

    fn event(&mut self, ev: UiEvent) -> bool {
        let handled = self.ui.event(&ev);
        handled | self.pump()
    }

    fn appearance_changed(&mut self, a: hey_style::Appearance) -> bool {
        if !self.auto_theme {
            // The theme is fixed by the app, but the user's text scale still applies.
            let factor = a.text_scale as f32 / 100.0;
            let scaled = self.ui.env.theme.clone().with_text_scale(factor);
            if scaled.text_scale == self.ui.env.theme.text_scale {
                return false;
            }
            self.ui.set_theme(scaled);
            self.dirty = true;
            return true;
        }
        self.ui.set_theme(Theme::for_appearance(a));
        self.dirty = true;
        true
    }

    fn inspector_key(&mut self, snapshot: bool) -> bool {
        if snapshot {
            self.ui.inspector_snapshot()
        } else {
            self.ui.set_inspector(!self.ui.inspector_enabled());
            true
        }
    }

    fn time_travel_key(&mut self, to: Seek) -> bool {
        self.time_travel_seek(to)
    }

    fn titlebar_dark(&self) -> Option<bool> {
        let bg = self.ui.env.theme.palette.desktop;
        // Rec. 601 luma of the desktop color.
        let luma = 0.299 * bg.r as f32 + 0.587 * bg.g as f32 + 0.114 * bg.b as f32;
        Some(luma < 128.0)
    }

    fn fonts(&self) -> Option<Arc<Fonts>> {
        Some(self.ui.env.fonts.clone())
    }

    fn images(&self) -> Option<Arc<hey_draw::ImageStore>> {
        Some(self.ui.env.icons.borrow().images())
    }

    fn ime_cursor_area(&self) -> Option<hey_core::Rect> {
        self.ui.ime_area()
    }

    fn a11y_tree(&mut self) -> Option<hey_widgets::A11yTree> {
        let mut t = self.ui.a11y_tree();
        let s = self.ui.scale() as f32;
        for n in &mut t.nodes {
            n.rect = hey_core::Rect::new(n.rect.x * s, n.rect.y * s, n.rect.w * s, n.rect.h * s);
        }
        Some(t)
    }

    fn a11y_action(&mut self, action: hey_widgets::A11yAction) -> bool {
        let handled = self.ui.a11y_action(action);
        handled | self.pump()
    }

    fn tick(&mut self) -> bool {
        let mut redraw = self.poll();
        if let Some(t) = self.watch.as_mut().and_then(ThemeWatch::changed) {
            self.ui.set_theme(t);
            self.dirty = true;
            redraw = true;
        }
        // Frame-independent animation time; capped so a long idle gap does not skip a transition.
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32() * 1000.0;
        self.last_tick = now;
        if self.ui.animating() {
            self.ui.tick(dt.min(50.0));
            redraw = true;
        }
        redraw
    }

    fn pending(&self) -> bool {
        self.watch.is_some() || self.ui.animating() || self.in_flight.load(Ordering::SeqCst) > 0
    }

    fn cursor(&self) -> hey_core::CursorIcon {
        self.ui.cursor()
    }
}
