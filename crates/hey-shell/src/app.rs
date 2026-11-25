// Copyright (c) Vitaly Golik (mjojo)

//! Public application API (MVU): `App::new(model).update(f).view(f).theme(t).run()`.
//!
//! Loop: event → `Ui` puts `Msg` into `MsgSink` → `update(&mut model, msg) -> Command` →
//! `view(&model)` → `Ui::reconcile`. Background tasks (`Command::perform`) run on threads,
//! results come back via `mpsc` and are polled in `tick()`.

use crate::{ShellApp, WindowConfig};
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
const DEFAULT_FONT: &[u8] = include_bytes!("../../../design/fonts/onest/Onest[wght].ttf");

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

/// Application builder.
pub struct App<M, Msg: Send + 'static> {
    model: M,
    update: Option<UpdateFn<M, Msg>>,
    view: Option<ViewFn<M, Msg>>,
    theme: Theme,
    fonts: Vec<PathBuf>,
    window: WindowConfig,
    init: Command<Msg>,
}

impl<M: 'static, Msg: Send + 'static> App<M, Msg> {
    pub fn new(model: M) -> Self {
        App {
            model,
            update: None,
            view: None,
            theme: Theme::dark(),
            fonts: Vec::new(),
            window: WindowConfig::default(),
            init: Command::None,
        }
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
    pub fn build(self) -> Result<Runtime<M, Msg>, String> {
        let update = self.update.ok_or("App: update is not set")?;
        let view = self.view.ok_or("App: view is not set")?;
        let mut fonts = Fonts::new();
        let mut chain: Vec<FontId> = Vec::new();
        for p in &self.fonts {
            chain.push(fonts.add_file(p).map_err(|e| format!("font {}: {e:?}", p.display()))?);
        }
        if chain.is_empty() {
            // Default font is Onest (OFL, Cyrillic), embedded into the binary.
            chain.push(fonts.add_bytes(DEFAULT_FONT).map_err(|e| format!("default font: {e:?}"))?);
        }
        let sink = MsgSink::new();
        let ui = Ui::new(self.theme, Arc::new(fonts), chain, sink.clone());
        let (tx, rx) = mpsc::channel();
        let mut rt = Runtime {
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
            dirty: true,
        };
        rt.run_command(self.init);
        rt.rebuild();
        Ok(rt)
    }

    pub fn run(self) -> Result<(), String> {
        let cfg =
            WindowConfig { title: self.window.title.clone(), size: self.window.size, clear: self.window.clear, vsync: self.window.vsync };
        let rt = self.build()?;
        crate::run(cfg, rt).map_err(|e| e.to_string())
    }
}

/// App runtime: implements `ShellApp`.
pub struct Runtime<M, Msg: Send + 'static> {
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
                let cmd = (self.update)(&mut self.model, m);
                self.run_command(cmd);
            }
        }
        if any {
            self.dirty = true;
        }
        any
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
        *list = self.frame(Size::new(viewport.width as f32, viewport.height as f32));
    }

    fn event(&mut self, ev: UiEvent) -> bool {
        let handled = self.ui.event(&ev);
        handled | self.pump()
    }

    fn fonts(&self) -> Option<Arc<Fonts>> {
        Some(self.ui.env.fonts.clone())
    }

    fn images(&self) -> Option<Arc<hey_draw::ImageStore>> {
        Some(self.ui.env.icons.borrow().images())
    }

    fn tick(&mut self) -> bool {
        self.poll()
    }

    fn pending(&self) -> bool {
        self.in_flight.load(Ordering::SeqCst) > 0
    }

    fn cursor(&self) -> hey_core::CursorIcon {
        self.ui.cursor()
    }
}
