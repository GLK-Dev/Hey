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
        let mut h = Self { ui, sink, viewport: Size::new(width, height), fonts };
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
        self.ui.event(&ev)
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
