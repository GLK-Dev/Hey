// Copyright (c) Vitaly Golik (mjojo)

//! hey-shell: window (winit) with a wgpu renderer, or the `CpuRenderer` through softbuffer.
//!
//! `HEY_RENDERER=cpu|gpu` picks the backend; by default the GPU is tried first and the CPU is the fallback.
//!
//! All sizes are in physical pixels (ADR-3); `scale` is passed to the app for reference.

mod a11y;
mod app;
mod clipboard;
mod dialog;
mod error;
mod input;
pub use app::{App, Command, TaskState};
pub use clipboard::{install_system_clipboard, SystemClipboard};
pub use dialog::{confirm, message, open_file, open_files, pick_folder, save_file};
pub use error::Error;
pub use input::{InputState, LINE_PX};

use hey_core::{Color, UiEvent};
use hey_draw::{DisplayList, UiRenderer, Viewport};
use hey_render_cpu::CpuRenderer;
use hey_render_gpu::GpuRenderer;
use hey_text::Fonts;
use std::{num::NonZeroU32, sync::Arc};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

mod os;

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes"))
}

/// Dark/light comes from the window system. High contrast and reduced motion come from the OS
/// where a native query exists (Windows); `HEY_HIGH_CONTRAST=1` / `HEY_REDUCE_MOTION=1` force
/// them on everywhere.
fn system_appearance(window: &Window) -> hey_style::Appearance {
    let (hc, rm) = os::native_flags().unwrap_or((false, false));
    hey_style::Appearance {
        dark: window.theme() != Some(winit::window::Theme::Light),
        high_contrast: hc || env_flag("HEY_HIGH_CONTRAST"),
        reduce_motion: rm || env_flag("HEY_REDUCE_MOTION"),
    }
}

/// App for the shell: builds a display list for every frame.
pub trait ShellApp {
    fn draw(&mut self, viewport: Viewport, list: &mut DisplayList);

    /// Normalized input event; return `true` to request a redraw.
    fn event(&mut self, _ev: UiEvent) -> bool {
        false
    }

    /// Font registry for rendering `DrawCmd::Text`; called when the window is created.
    fn fonts(&self) -> Option<std::sync::Arc<hey_text::Fonts>> {
        None
    }

    /// Image store for `DrawCmd::Image`; re-read after every `draw` (a changed `Arc` is re-uploaded).
    fn images(&self) -> Option<std::sync::Arc<hey_draw::ImageStore>> {
        None
    }

    /// The system appearance (dark/light, high contrast, reduced motion) is known or has changed.
    /// Called once after the window is created and on every system theme switch; `true` requests a redraw.
    fn appearance_changed(&mut self, _a: hey_style::Appearance) -> bool {
        false
    }

    /// Whether the app's current theme is dark, so the shell can match the native title bar to it
    /// (Windows). `None` leaves the title bar to the OS.
    fn titlebar_dark(&self) -> Option<bool> {
        None
    }

    /// Caret rectangle in physical pixels for the IME candidate window (`None` — no text input focused).
    fn ime_cursor_area(&self) -> Option<hey_core::Rect> {
        None
    }

    /// Called when the loop wakes up: collect background task results; `true` means a redraw is needed.
    /// Accessibility snapshot of the last laid-out frame (`None` — accessibility is not supported).
    fn a11y_tree(&mut self) -> Option<hey_widgets::A11yTree> {
        None
    }

    /// Performs an action requested by assistive technology; `true` means a redraw is needed.
    fn a11y_action(&mut self, _action: hey_widgets::A11yAction) -> bool {
        false
    }

    fn tick(&mut self) -> bool {
        false
    }

    /// Whether background work is running: then the loop polls `tick` every ~16 ms instead of waiting for events.
    fn pending(&self) -> bool {
        false
    }

    /// Which cursor to show now (after events are processed).
    fn cursor(&self) -> hey_core::CursorIcon {
        hey_core::CursorIcon::Default
    }
}

fn cursor_icon(c: hey_core::CursorIcon) -> winit::window::CursorIcon {
    use hey_core::CursorIcon as C;
    use winit::window::CursorIcon as W;
    match c {
        C::Default => W::Default,
        C::Pointer => W::Pointer,
        C::Text => W::Text,
        C::NotAllowed => W::NotAllowed,
        C::ResizeH => W::EwResize,
        C::ResizeV => W::NsResize,
    }
}

pub struct WindowConfig {
    pub title: String,
    /// Logical size at creation.
    pub size: (f64, f64),
    pub clear: hey_core::Color,
    /// Wait for vertical sync on the GPU backend (`HEY_VSYNC=0|1` overrides).
    pub vsync: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: "Hey".into(), size: (960.0, 600.0), clear: hey_core::Color::hex(0x0B0C10), vsync: true }
    }
}

pub fn run<A: ShellApp>(cfg: WindowConfig, app: A) -> Result<(), Error> {
    #[cfg(feature = "profiling")]
    {
        use tracing_subscriber::fmt::format::FmtSpan;
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "hey_shell=info,hey_widgets=info".into()),
            )
            .with_span_events(FmtSpan::CLOSE)
            .with_writer(std::io::stderr)
            .try_init();
    }
    install_system_clipboard();
    let event_loop = EventLoop::<a11y::UserEvent>::with_user_event().build().map_err(|e| Error::EventLoop(e.to_string()))?;
    let proxy = event_loop.create_proxy();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut shell = Shell { cfg, app, gfx: None, list: DisplayList::new(), input: InputState::default(), adapter: None, proxy };
    event_loop.run_app(&mut shell).map_err(|e| Error::EventLoop(e.to_string()))
}

enum Backend {
    Cpu { surface: softbuffer::Surface<Arc<Window>, Arc<Window>>, renderer: CpuRenderer },
    Gpu(Box<GpuRenderer>),
}

struct Gfx {
    window: Arc<Window>,
    /// `None` only while switching backends (the old one must be dropped before the new surface is created).
    backend: Option<Backend>,
    /// Image store last handed to the active renderer (compared by pointer to detect new icons).
    images: Option<Arc<hey_draw::ImageStore>>,
    /// Window is fully covered or minimized: skip rendering until it becomes visible again.
    occluded: bool,
}

fn create_gpu(window: &Arc<Window>, clear: Color, fonts: Option<Arc<Fonts>>, vsync: bool) -> Result<Backend, Error> {
    let sz = window.inner_size();
    let mut r = GpuRenderer::new_for_window(window.clone(), sz.width.max(1), sz.height.max(1)).map_err(|e| Error::Gpu(e.to_string()))?;
    r.clear_color = clear;
    r.fonts = fonts;
    r.set_vsync(vsync);
    Ok(Backend::Gpu(Box::new(r)))
}

fn create_cpu(window: &Arc<Window>, clear: Color, fonts: Option<Arc<Fonts>>) -> Backend {
    let sz = window.inner_size();
    let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
    let surface = softbuffer::Surface::new(&ctx, window.clone()).expect("softbuffer surface");
    let mut renderer = CpuRenderer::new(sz.width.max(1), sz.height.max(1));
    renderer.clear_color = clear;
    renderer.fonts = fonts;
    Backend::Cpu { surface, renderer }
}

/// Dev hotkey Ctrl+Shift+F12: toggles between the GPU and CPU renderers at runtime.
fn is_switch_hotkey(event: &WindowEvent, mods: hey_core::Modifiers) -> bool {
    matches!(event, WindowEvent::KeyboardInput { event: k, .. }
        if k.state == ElementState::Pressed && !k.repeat && k.logical_key == Key::Named(NamedKey::F12))
        && mods.ctrl
        && mods.shift
}

impl Gfx {
    fn switch_backend(&mut self, clear: Color, fonts: Option<Arc<Fonts>>, vsync: bool) {
        let to_gpu = matches!(self.backend, Some(Backend::Cpu { .. }));
        self.images = None; // the new renderer has no images yet
        self.backend = None; // release the current surface first: one window cannot hold two
        let created =
            if to_gpu { create_gpu(&self.window, clear, fonts.clone(), vsync) } else { Ok(create_cpu(&self.window, clear, fonts.clone())) };
        self.backend = Some(match created {
            Ok(b) => b,
            Err(e) => {
                eprintln!("hey: cannot switch to the GPU renderer ({e}), staying on the CPU");
                create_cpu(&self.window, clear, fonts)
            }
        });
        eprintln!("hey: renderer = {}", if matches!(self.backend, Some(Backend::Gpu(_))) { "gpu" } else { "cpu" });
        self.window.request_redraw();
    }
}

/// Effective vsync: `HEY_VSYNC=0|1` overrides the configured value.
fn vsync_setting(configured: bool) -> bool {
    match std::env::var("HEY_VSYNC").as_deref() {
        Ok("0") => false,
        Ok("1") => true,
        _ => configured,
    }
}

/// Which backends may be used, from `HEY_RENDERER` (`cpu`, `gpu`; anything else = auto).
fn renderer_pref() -> (bool, bool) {
    match std::env::var("HEY_RENDERER").as_deref() {
        Ok("cpu") => (false, true),
        Ok("gpu") => (true, false),
        _ => (true, true),
    }
}

struct Shell<A> {
    cfg: WindowConfig,
    app: A,
    gfx: Option<Gfx>,
    list: DisplayList,
    input: InputState,
    adapter: Option<accesskit_winit::Adapter>,
    proxy: winit::event_loop::EventLoopProxy<a11y::UserEvent>,
}

impl<A: ShellApp> ApplicationHandler<a11y::UserEvent> for Shell<A> {
    fn user_event(&mut self, _el: &ActiveEventLoop, event: a11y::UserEvent) {
        let a11y::UserEvent::Access(ev) = event;
        let title = self.cfg.title.clone();
        match ev.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                let upd = self.app.a11y_tree().map(|t| a11y::to_update(&t, &title)).unwrap_or_else(|| a11y::placeholder(&title));
                if let Some(ad) = self.adapter.as_mut() {
                    ad.update_if_active(|| upd);
                }
            }
            accesskit_winit::WindowEvent::ActionRequested(req) => {
                if let Some(a) = a11y::action(&req) {
                    if self.app.a11y_action(a) {
                        if let Some(g) = &self.gfx {
                            g.window.request_redraw();
                        }
                    }
                }
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Read pending BEFORE tick: a task sends its result before it decrements the counter,
        // so pending == false guarantees that tick has already seen all results.
        let pending = self.app.pending();
        if self.app.tick() {
            if let Some(g) = &self.gfx {
                g.window.request_redraw();
            }
        }
        el.set_control_flow(if pending {
            ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(16))
        } else {
            ControlFlow::Wait
        });
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.cfg.title.clone())
            .with_inner_size(LogicalSize::new(self.cfg.size.0, self.cfg.size.1))
            .with_visible(false);
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("hey: cannot create the window: {e}");
                el.exit();
                return;
            }
        };
        // The adapter must exist before the window becomes visible.
        self.adapter = Some(accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.proxy.clone()));
        window.set_visible(true);
        let (try_gpu, allow_cpu) = renderer_pref();
        let (clear, fonts) = (self.cfg.clear, self.app.fonts());
        let mut backend = None;
        if try_gpu {
            match create_gpu(&window, clear, fonts.clone(), vsync_setting(self.cfg.vsync)) {
                Ok(b) => backend = Some(b),
                Err(e) if allow_cpu => eprintln!("hey: GPU unavailable ({e}), using the CPU renderer"),
                Err(e) => panic!("hey: GPU renderer requested but unavailable: {e}"),
            }
        }
        let backend = backend.unwrap_or_else(|| create_cpu(&window, clear, fonts));
        window.set_ime_allowed(true);
        self.app.appearance_changed(system_appearance(&window));
        if let Some(dark) = self.app.titlebar_dark() {
            os::set_dark_titlebar(&window, dark);
        }
        window.request_redraw();
        self.gfx = Some(Gfx { window, backend: Some(backend), images: None, occluded: false });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(g) = self.gfx.as_mut() else { return };
        if let Some(ad) = self.adapter.as_mut() {
            ad.process_event(&g.window, &event);
        }
        if is_switch_hotkey(&event, self.input.mods) {
            g.switch_backend(self.cfg.clear, self.app.fonts(), vsync_setting(self.cfg.vsync));
            return;
        }
        let mut redraw = false;
        self.input.scale = g.window.scale_factor();
        for ev in self.input.translate(&event) {
            redraw |= self.app.event(ev);
        }
        if redraw {
            g.window.request_redraw();
        }
        g.window.set_cursor(cursor_icon(self.app.cursor()));
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Occluded(hidden) => {
                g.occluded = hidden;
                if !hidden {
                    g.window.request_redraw();
                }
            }
            WindowEvent::ThemeChanged(_) => {
                if self.app.appearance_changed(system_appearance(&g.window)) {
                    g.window.request_redraw();
                }
                if let Some(dark) = self.app.titlebar_dark() {
                    os::set_dark_titlebar(&g.window, dark);
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => g.window.request_redraw(),
            WindowEvent::RedrawRequested => {
                if g.occluded {
                    return;
                }
                let sz = g.window.inner_size();
                let (Some(w), Some(h)) = (NonZeroU32::new(sz.width), NonZeroU32::new(sz.height)) else {
                    return; // minimized
                };
                let vp = Viewport { width: sz.width, height: sz.height, scale: g.window.scale_factor() };
                let _frame = tracing::info_span!("frame").entered();
                self.list.clear();
                {
                    let _span = tracing::info_span!("app_draw").entered();
                    self.app.draw(vp, &mut self.list);
                }
                if let Some(ad) = self.adapter.as_mut() {
                    // The snapshot is only built while assistive technology is connected.
                    let (app, title) = (&mut self.app, &self.cfg.title);
                    ad.update_if_active(|| app.a11y_tree().map(|t| a11y::to_update(&t, title)).unwrap_or_else(|| a11y::placeholder(title)));
                }
                if let Some(r) = self.app.ime_cursor_area() {
                    g.window.set_ime_cursor_area(
                        winit::dpi::PhysicalPosition::new(r.x as f64, r.y as f64),
                        winit::dpi::PhysicalSize::new(r.w as f64, r.h as f64),
                    );
                }
                let images = self.app.images();
                let changed = match (&images, &g.images) {
                    (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                    (None, None) => false,
                    _ => true,
                };
                if changed {
                    match g.backend.as_mut().expect("backend") {
                        Backend::Gpu(r) => r.set_images(images.clone()),
                        Backend::Cpu { renderer, .. } => renderer.images = images.clone(),
                    }
                    g.images = images;
                }
                let _render = tracing::info_span!("render").entered();
                match g.backend.as_mut().expect("backend") {
                    Backend::Gpu(r) => {
                        r.resize(sz.width, sz.height);
                        r.render(&self.list, vp);
                    }
                    Backend::Cpu { surface, renderer } => {
                        if (renderer.fb.width, renderer.fb.height) != (sz.width, sz.height) {
                            renderer.resize(sz.width, sz.height);
                        }
                        renderer.render(&self.list, vp);
                        // A lost or resizing surface only drops this frame; the next redraw retries.
                        if surface.resize(w, h).is_err() {
                            return;
                        }
                        let Ok(mut buf) = surface.buffer_mut() else { return };
                        for (dst, px) in buf.iter_mut().zip(renderer.fb.data.as_chunks::<4>().0.iter()) {
                            *dst = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
                        }
                        if let Err(e) = buf.present() {
                            eprintln!("hey: present failed: {e}");
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
