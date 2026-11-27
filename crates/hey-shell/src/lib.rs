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
/// Version of the Hey framework (the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use app::{App, Command, Seek, TaskState};
pub use clipboard::{install_system_clipboard, SystemClipboard};
pub use dialog::{confirm, message, open_file, open_files, pick_folder, save_file};
pub use error::Error;
pub use input::{InputState, LINE_PX};

use hey_core::{Color, UiEvent};
use hey_draw::{DisplayList, UiRenderer, Viewport};
use hey_render_cpu::CpuRenderer;
#[cfg(feature = "gpu")]
use hey_render_gpu::GpuRenderer;
use hey_text::Fonts;
use std::{collections::HashMap, num::NonZeroU32, sync::Arc};
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

/// System text scale in percent: `HEY_TEXT_SCALE` (e.g. `1.5` or `150`) overrides the OS value.
fn text_scale_percent() -> u16 {
    let env = std::env::var("HEY_TEXT_SCALE").ok().and_then(|v| v.trim().parse::<f32>().ok());
    let pct = match env {
        Some(v) if v > 0.0 && v <= 10.0 => v * 100.0,
        Some(v) if v > 0.0 => v,
        _ => os::system_text_scale() * 100.0,
    };
    pct.round().clamp(50.0, 400.0) as u16
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
        text_scale: text_scale_percent(),
    }
}

/// Identifies a window of the app. The first window is [`MAIN_WINDOW`]; extra windows get keys chosen by the app.
pub type WindowKey = u32;

/// Key of the first window; closing it exits the event loop.
pub const MAIN_WINDOW: WindowKey = 0;

/// App for the shell: builds a display list for every frame.
///
/// Single-window apps ignore the multi-window hooks ([`focus_window`](ShellApp::focus_window),
/// [`take_new_windows`](ShellApp::take_new_windows), [`window_closed`](ShellApp::window_closed)).
pub trait ShellApp {
    /// The shell is about to call `draw`, `event`, `cursor`, `a11y_*` and the like on behalf of this window.
    fn focus_window(&mut self, _key: WindowKey) {}

    /// Extra windows the app wants opened; polled after every event and tick. Keys must be unique and not [`MAIN_WINDOW`].
    fn take_new_windows(&mut self) -> Vec<(WindowKey, WindowConfig)> {
        Vec::new()
    }

    /// An extra window was closed by the user (closing [`MAIN_WINDOW`] exits instead).
    fn window_closed(&mut self, _key: WindowKey) {}

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

    /// Dev hotkeys: Ctrl+Shift+F10 toggles the display-list inspector (`snapshot == false`),
    /// Ctrl+Shift+F9 pins the current frame as its diff baseline (`snapshot == true`).
    /// `true` requests a redraw.
    fn inspector_key(&mut self, _snapshot: bool) -> bool {
        false
    }

    /// Dev hotkeys Ctrl+Shift+Left / Right / Home / End step the time-travel history
    /// (`App::time_travel`). `true` requests a redraw.
    fn time_travel_key(&mut self, _to: Seek) -> bool {
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
    let mut shell = Shell { cfg, app, windows: HashMap::new(), proxy };
    event_loop.run_app(&mut shell).map_err(|e| Error::EventLoop(e.to_string()))
}

enum Backend {
    Cpu {
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
        renderer: CpuRenderer,
    },
    #[cfg(feature = "gpu")]
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

#[cfg(not(feature = "gpu"))]
fn create_gpu(_window: &Arc<Window>, _clear: Color, _fonts: Option<Arc<Fonts>>, _vsync: bool) -> Result<Backend, Error> {
    Err(Error::Gpu("built without the `gpu` feature".into()))
}

#[cfg(feature = "gpu")]
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

/// Dev hotkey Ctrl+Shift+F11: starts or stops recording the window's input to a `.hey-session` file.
fn is_record_hotkey(event: &WindowEvent, mods: hey_core::Modifiers) -> bool {
    matches!(event, WindowEvent::KeyboardInput { event: k, .. }
        if k.state == ElementState::Pressed && !k.repeat && k.logical_key == Key::Named(NamedKey::F11))
        && mods.ctrl
        && mods.shift
}

/// Dev hotkeys Ctrl+Shift+F10 (toggle inspector) and Ctrl+Shift+F9 (pin snapshot): `Some(is_snapshot)`.
fn inspector_hotkey(event: &WindowEvent, mods: hey_core::Modifiers) -> Option<bool> {
    let WindowEvent::KeyboardInput { event: k, .. } = event else { return None };
    if k.state != ElementState::Pressed || k.repeat || !mods.ctrl || !mods.shift {
        return None;
    }
    match k.logical_key {
        Key::Named(NamedKey::F10) => Some(false),
        Key::Named(NamedKey::F9) => Some(true),
        _ => None,
    }
}

/// Dev hotkeys Ctrl+Shift+Left / Right / Home / End (key repeat scrubs).
fn time_travel_hotkey(event: &WindowEvent, mods: hey_core::Modifiers) -> Option<Seek> {
    let WindowEvent::KeyboardInput { event: k, .. } = event else { return None };
    if k.state != ElementState::Pressed || !mods.ctrl || !mods.shift {
        return None;
    }
    match k.logical_key {
        Key::Named(NamedKey::ArrowLeft) => Some(Seek::Back),
        Key::Named(NamedKey::ArrowRight) => Some(Seek::Forward),
        Key::Named(NamedKey::Home) => Some(Seek::Start),
        Key::Named(NamedKey::End) => Some(Seek::Live),
        _ => None,
    }
}

/// Output path for a finished recording: `$HEY_SESSION_FILE`, else `hey-<unix seconds>.hey-session`.
fn session_path() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("HEY_SESSION_FILE") {
        return p.into();
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format!("hey-{secs}.hey-session").into()
}

impl Win {
    /// Start a capture (seeded with the current size and scale so a replay lays out identically),
    /// or stop it and write the file.
    fn toggle_recording(&mut self) {
        if let Some((_, rec)) = self.recording.take() {
            let path = session_path();
            match rec.save(&path) {
                Ok(()) => eprintln!("hey: session saved to {}", path.display()),
                Err(e) => eprintln!("hey: cannot save session to {}: {e}", path.display()),
            }
            return;
        }
        let sz = self.gfx.window.inner_size();
        let mut rec = hey_core::Recording::new();
        rec.push_event(0, UiEvent::Resize { width: sz.width, height: sz.height });
        rec.push_event(0, UiEvent::ScaleFactor(self.gfx.window.scale_factor()));
        self.recording = Some((std::time::Instant::now(), rec));
        eprintln!("hey: recording session (Ctrl+Shift+F11 to stop)");
    }

    fn record(&mut self, ev: &UiEvent) {
        if let Some((t0, rec)) = &mut self.recording {
            rec.push_event(t0.elapsed().as_millis() as u64, ev.clone());
        }
    }
}

impl Gfx {
    fn is_gpu(&self) -> bool {
        #[cfg(feature = "gpu")]
        {
            matches!(self.backend, Some(Backend::Gpu(_)))
        }
        #[cfg(not(feature = "gpu"))]
        {
            false
        }
    }

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
        eprintln!("hey: renderer = {}", if self.is_gpu() { "gpu" } else { "cpu" });
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

/// One OS window with its own renderer, input state and accessibility adapter.
struct Win {
    key: WindowKey,
    title: String,
    clear: Color,
    vsync: bool,
    gfx: Gfx,
    list: DisplayList,
    input: InputState,
    adapter: accesskit_winit::Adapter,
    /// Live session capture (Ctrl+Shift+F11), see [`is_record_hotkey`].
    recording: Option<(std::time::Instant, hey_core::Recording)>,
}

struct Shell<A> {
    cfg: WindowConfig,
    app: A,
    windows: HashMap<WindowId, Win>,
    proxy: winit::event_loop::EventLoopProxy<a11y::UserEvent>,
}

impl<A: ShellApp> Shell<A> {
    fn create_win(&mut self, el: &ActiveEventLoop, key: WindowKey, cfg: &WindowConfig) -> bool {
        let attrs = Window::default_attributes()
            .with_title(cfg.title.clone())
            .with_inner_size(LogicalSize::new(cfg.size.0, cfg.size.1))
            .with_visible(false);
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("hey: cannot create the window: {e}");
                return false;
            }
        };
        // The adapter must exist before the window becomes visible.
        let adapter = accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.proxy.clone());
        window.set_visible(true);
        let (try_gpu, allow_cpu) = renderer_pref();
        let (clear, fonts) = (cfg.clear, self.app.fonts());
        let mut backend = None;
        if try_gpu {
            match create_gpu(&window, clear, fonts.clone(), vsync_setting(cfg.vsync)) {
                Ok(b) => backend = Some(b),
                Err(e) if allow_cpu => eprintln!("hey: GPU unavailable ({e}), using the CPU renderer"),
                Err(e) => panic!("hey: GPU renderer requested but unavailable: {e}"),
            }
        }
        let backend = backend.unwrap_or_else(|| create_cpu(&window, clear, fonts));
        window.set_ime_allowed(true);
        self.app.focus_window(key);
        self.app.appearance_changed(system_appearance(&window));
        if let Some(dark) = self.app.titlebar_dark() {
            os::set_dark_titlebar(&window, dark);
        }
        window.request_redraw();
        let id = window.id();
        let gfx = Gfx { window, backend: Some(backend), images: None, occluded: false };
        let win = Win {
            key,
            title: cfg.title.clone(),
            clear,
            vsync: cfg.vsync,
            gfx,
            list: DisplayList::new(),
            input: InputState::default(),
            adapter,
            recording: None,
        };
        self.windows.insert(id, win);
        true
    }

    /// Opens the windows the app asked for.
    fn spawn_requested(&mut self, el: &ActiveEventLoop) {
        for (key, cfg) in self.app.take_new_windows() {
            if key == MAIN_WINDOW || self.windows.values().any(|w| w.key == key) {
                eprintln!("hey: window key {key} is already in use");
                continue;
            }
            self.create_win(el, key, &cfg);
        }
    }

    fn redraw_all(&self) {
        for w in self.windows.values() {
            w.gfx.window.request_redraw();
        }
    }
}

impl<A: ShellApp> ApplicationHandler<a11y::UserEvent> for Shell<A> {
    fn user_event(&mut self, _el: &ActiveEventLoop, event: a11y::UserEvent) {
        let a11y::UserEvent::Access(ev) = event;
        let Some(win) = self.windows.get_mut(&ev.window_id) else { return };
        self.app.focus_window(win.key);
        let title = win.title.clone();
        match ev.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                let upd = self.app.a11y_tree().map(|t| a11y::to_update(&t, &title)).unwrap_or_else(|| a11y::placeholder(&title));
                win.adapter.update_if_active(|| upd);
            }
            accesskit_winit::WindowEvent::ActionRequested(req) => {
                if let Some(a) = a11y::action(&req) {
                    if self.app.a11y_action(a) {
                        win.gfx.window.request_redraw();
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
            self.redraw_all();
        }
        self.spawn_requested(el);
        el.set_control_flow(if pending {
            ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(16))
        } else {
            ControlFlow::Wait
        });
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }
        let cfg = WindowConfig { title: self.cfg.title.clone(), size: self.cfg.size, clear: self.cfg.clear, vsync: self.cfg.vsync };
        if !self.create_win(el, MAIN_WINDOW, &cfg) {
            el.exit();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(g) = self.windows.get_mut(&id) else { return };
        self.app.focus_window(g.key);
        g.adapter.process_event(&g.gfx.window, &event);
        if is_switch_hotkey(&event, g.input.mods) {
            g.gfx.switch_backend(g.clear, self.app.fonts(), vsync_setting(g.vsync));
            return;
        }
        if is_record_hotkey(&event, g.input.mods) {
            g.toggle_recording();
            return;
        }
        if let Some(snapshot) = inspector_hotkey(&event, g.input.mods) {
            if self.app.inspector_key(snapshot) {
                g.gfx.window.request_redraw();
            }
            return;
        }
        if let Some(to) = time_travel_hotkey(&event, g.input.mods) {
            if self.app.time_travel_key(to) {
                g.gfx.window.request_redraw();
            }
            return;
        }
        let mut redraw = false;
        g.input.scale = g.gfx.window.scale_factor();
        for ev in g.input.translate(&event) {
            g.record(&ev);
            redraw |= self.app.event(ev);
        }
        g.gfx.window.set_cursor(cursor_icon(self.app.cursor()));
        match event {
            WindowEvent::CloseRequested => {
                if g.key == MAIN_WINDOW {
                    el.exit();
                } else {
                    let key = g.key;
                    self.windows.remove(&id);
                    self.app.window_closed(key);
                    self.redraw_all();
                }
                return;
            }
            WindowEvent::Occluded(hidden) => {
                g.gfx.occluded = hidden;
                redraw |= !hidden;
            }
            // The text scale has no change notification; re-read it when the user returns to the window.
            WindowEvent::Focused(true) | WindowEvent::ThemeChanged(_) => {
                redraw |= self.app.appearance_changed(system_appearance(&g.gfx.window));
                if let Some(dark) = self.app.titlebar_dark() {
                    os::set_dark_titlebar(&g.gfx.window, dark);
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => redraw = true,
            WindowEvent::RedrawRequested => {
                let Win { gfx: g, list, adapter, title, .. } = g;
                if g.occluded {
                    return;
                }
                let sz = g.window.inner_size();
                let (Some(w), Some(h)) = (NonZeroU32::new(sz.width), NonZeroU32::new(sz.height)) else {
                    return; // minimized
                };
                let vp = Viewport { width: sz.width, height: sz.height, scale: g.window.scale_factor() };
                let _frame = tracing::info_span!("frame").entered();
                list.clear();
                {
                    let _span = tracing::info_span!("app_draw").entered();
                    self.app.draw(vp, list);
                }
                {
                    // The snapshot is only built while assistive technology is connected.
                    let app = &mut self.app;
                    adapter.update_if_active(|| {
                        app.a11y_tree().map(|t| a11y::to_update(&t, title)).unwrap_or_else(|| a11y::placeholder(title))
                    });
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
                        #[cfg(feature = "gpu")]
                        Backend::Gpu(r) => r.set_images(images.clone()),
                        Backend::Cpu { renderer, .. } => renderer.images = images.clone(),
                    }
                    g.images = images;
                }
                let _render = tracing::info_span!("render").entered();
                match g.backend.as_mut().expect("backend") {
                    #[cfg(feature = "gpu")]
                    Backend::Gpu(r) => {
                        r.resize(sz.width, sz.height);
                        r.render(list, vp);
                    }
                    Backend::Cpu { surface, renderer } => {
                        if (renderer.fb.width, renderer.fb.height) != (sz.width, sz.height) {
                            renderer.resize(sz.width, sz.height);
                        }
                        renderer.render(list, vp);
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
                return;
            }
            _ => {}
        }
        if redraw {
            // An event may change state shown by other windows too.
            self.redraw_all();
        }
        self.spawn_requested(el);
    }
}
