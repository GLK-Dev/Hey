// Copyright (c) Vitaly Golik (mjojo)

//! Headless test harness: drive a [`Ui`] with simulated input and compare
//! rendered frames against golden PNGs (CPU renderer is the reference, ADR-6).
//!
//! ```ignore
//! let mut h = Harness::<String>::new(400.0, 100.0);
//! let input = TextInput::add(&mut h.ui, h.ui.root(), "", 200.0, Callback::new(|s| s));
//! h.click(input);
//! h.type_text("hello");
//! h.undo();
//! h.screenshot(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden"), "after_undo");
//! ```

mod audit;

pub use audit::{audit_tree, Issue, IssueKind, MIN_TARGET};
pub use hey_core::{Entry, ParseError, Recording};

/// A replayed frame hash differed from the recorded one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayMismatch {
    /// Index into [`Recording::entries`].
    pub entry: usize,
    /// Recording timestamp in milliseconds.
    pub ms: u64,
    pub expected: u64,
    pub actual: u64,
}

impl std::fmt::Display for ReplayMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "frame hash differs at entry {} ({} ms): recorded {:016x}, replayed {:016x}",
            self.entry, self.ms, self.expected, self.actual
        )
    }
}

impl std::error::Error for ReplayMismatch {}

use hey_core::{Color, Modifiers, MouseButton, MsgSink, NodeId, Point, Size, UiEvent};
use hey_draw::{UiRenderer, Viewport};
use hey_render_cpu::{CpuRenderer, Framebuffer};
use hey_style::Theme;
use hey_text::Fonts;
use hey_widgets::Ui;
use std::path::Path;
use std::sync::Arc;

/// A [`Ui`] plus a message sink, viewport and renderer.
pub struct Harness<Msg: 'static> {
    pub ui: Ui<Msg>,
    pub sink: MsgSink<Msg>,
    pub viewport: Size,
    fonts: Arc<Fonts>,
    clock_ms: u64,
    recording: Option<Recording>,
}

impl<Msg: 'static> Harness<Msg> {
    /// Dark theme, bundled font, given viewport in physical pixels.
    pub fn new(width: f32, height: f32) -> Self {
        Self::with_theme(Theme::dark(), width, height)
    }

    pub fn with_theme(theme: Theme, width: f32, height: f32) -> Self {
        let mut f = Fonts::new();
        let id = f.add_bytes(hey_text::DEFAULT_FONT).expect("bundled font");
        let fonts = Arc::new(f);
        let sink = MsgSink::new();
        let ui = Ui::new(theme, fonts.clone(), vec![id], sink.clone());
        let mut h = Self { ui, sink, viewport: Size::new(width, height), fonts, clock_ms: 0, recording: None };
        h.frame();
        h
    }

    /// Set the DPI scale factor; the viewport stays in physical pixels.
    pub fn set_scale(&mut self, scale: f64) {
        self.ui.set_scale(scale);
        self.frame();
    }

    /// Accessibility lints over the current tree: unnamed, undersized or keyboard-unreachable controls.
    /// An empty list means the UI passes; use `assert!(h.audit().is_empty(), ...)` in tests.
    pub fn audit(&mut self) -> Vec<Issue> {
        self.frame();
        audit_tree(&self.ui.a11y_tree())
    }

    /// Run layout and paint once (needed before hit-testing new nodes).
    pub fn frame(&mut self) {
        self.ui.paint(self.viewport);
    }

    /// Dispatch a raw event; returns whether it was handled.
    pub fn send(&mut self, ev: UiEvent) -> bool {
        self.frame();
        if let Some(rec) = &mut self.recording {
            rec.push_event(self.clock_ms, ev.clone());
        }
        self.ui.event(&ev)
    }

    /// Advance the simulated clock and animations (hover fades, ...) by `ms`.
    pub fn advance(&mut self, ms: u64) {
        self.clock_ms += ms;
        self.ui.tick(ms as f32);
        self.frame();
    }

    /// Start recording every event sent through the harness, with timestamps from [`advance`](Self::advance).
    /// Events already sent are not included; start before the first interaction.
    pub fn start_recording(&mut self) {
        self.recording = Some(Recording::new());
    }

    /// Stop recording and return the session (empty if recording was never started).
    pub fn stop_recording(&mut self) -> Recording {
        self.recording.take().unwrap_or_default()
    }

    /// Hash of the current frame (FNV-1a of the CPU-rendered pixels): stable across platforms.
    pub fn frame_hash(&mut self) -> u64 {
        hey_core::fnv1a(&self.render().data)
    }

    /// While recording, store the current frame hash so a replay can verify it.
    pub fn checkpoint(&mut self) {
        if self.recording.is_some() {
            let h = self.frame_hash();
            let ms = self.clock_ms;
            if let Some(rec) = &mut self.recording {
                rec.push_hash(ms, h);
            }
        }
    }

    /// Feed a recording through the UI at its original timing. Every `hash` checkpoint must match
    /// the replayed frame; the first difference is returned.
    pub fn replay(&mut self, rec: &Recording) -> Result<(), ReplayMismatch> {
        for (i, (ms, entry)) in rec.entries.iter().enumerate() {
            if *ms > self.clock_ms {
                self.advance(*ms - self.clock_ms);
            }
            match entry {
                Entry::Event(ev) => {
                    if let UiEvent::Resize { width, height } = ev {
                        self.viewport = Size::new(*width as f32, *height as f32);
                    }
                    self.send(ev.clone());
                }
                Entry::Hash(want) => {
                    let got = self.frame_hash();
                    if got != *want {
                        return Err(ReplayMismatch { entry: i, ms: *ms, expected: *want, actual: got });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Load a recording file and [`replay`](Self::replay) it.
    pub fn replay_file(&mut self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let rec = Recording::load(path)?;
        Ok(self.replay(&rec)?)
    }

    /// Move the pointer to the centre of `node`, then press and release.
    pub fn click(&mut self, node: NodeId) {
        self.frame();
        let r = self.ui.tree.rect(node);
        self.click_at(Point::new(r.x + r.w * 0.5, r.y + r.h * 0.5));
    }

    pub fn click_at(&mut self, pos: Point) {
        self.send(UiEvent::PointerMove { pos });
        self.send(UiEvent::PointerDown { pos, button: MouseButton::Left });
        self.send(UiEvent::PointerUp { pos, button: MouseButton::Left });
    }

    pub fn type_text(&mut self, s: &str) {
        self.send(UiEvent::Text(s.into()));
    }

    pub fn key(&mut self, code: &str, mods: Modifiers) -> bool {
        self.send(UiEvent::KeyDown { code: code.into(), mods })
    }

    pub fn press(&mut self, code: &str) -> bool {
        self.key(code, Modifiers::default())
    }

    /// Platform command shortcut (Ctrl, or Cmd on macOS) with a letter key.
    pub fn command(&mut self, code: &str, shift: bool) -> bool {
        let mut mods = Modifiers { shift, ..Default::default() };
        if cfg!(target_os = "macos") {
            mods.logo = true;
        } else {
            mods.ctrl = true;
        }
        self.key(code, mods)
    }

    pub fn undo(&mut self) -> bool {
        self.command("z", false)
    }

    pub fn redo(&mut self) -> bool {
        self.command("z", true)
    }

    /// Messages emitted since the last call.
    pub fn messages(&mut self) -> Vec<Msg> {
        self.sink.drain()
    }

    /// Render the current frame with the CPU renderer.
    pub fn render(&mut self) -> Framebuffer {
        let list = self.ui.paint(self.viewport);
        let (w, h) = (self.viewport.w as u32, self.viewport.h as u32);
        let mut r = CpuRenderer::new(w, h);
        r.fonts = Some(self.fonts.clone());
        r.clear_color = Color::hex(0x0B0C10);
        r.render(&list, Viewport { width: w, height: h, scale: 1.0 });
        r.fb
    }

    /// Compare the frame with `<dir>/<name>.png`. A missing reference, or
    /// `HEY_UPDATE_GOLDEN=1`, writes it instead.
    pub fn screenshot(&mut self, dir: impl AsRef<Path>, name: &str) {
        let fb = self.render();
        let path = dir.as_ref().join(format!("{name}.png"));
        if std::env::var_os("HEY_UPDATE_GOLDEN").is_some() || !path.exists() {
            std::fs::create_dir_all(dir.as_ref()).unwrap();
            fb.save_png(&path).unwrap();
            return;
        }
        let want = Framebuffer::decode_png(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((want.width, want.height), (fb.width, fb.height), "{name}: size");
        assert!(want.data == fb.data, "{name}: frame differs from the reference (HEY_UPDATE_GOLDEN=1 to update)");
    }
}
