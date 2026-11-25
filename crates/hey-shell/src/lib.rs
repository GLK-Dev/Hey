// Copyright (c) Vitaly Golik (mjojo)

//! hey-shell: window (winit) and `CpuRenderer` frame output through softbuffer.
//!
//! All sizes are in physical pixels (ADR-3); `scale` is passed to the app for reference.

mod app;
mod clipboard;
mod dialog;
mod input;
pub use app::{App, Command, TaskState};
pub use clipboard::{install_system_clipboard, SystemClipboard};
pub use dialog::{confirm, message, open_file, open_files, pick_folder, save_file};
pub use input::{InputState, LINE_PX};

use hey_core::UiEvent;
use hey_draw::{DisplayList, UiRenderer, Viewport};
use hey_render_cpu::CpuRenderer;
use std::{num::NonZeroU32, rc::Rc};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

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

    /// Called when the loop wakes up: collect background task results; `true` means a redraw is needed.
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
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: "Hey".into(), size: (960.0, 600.0), clear: hey_core::Color::hex(0x0B0C10) }
    }
}

pub fn run<A: ShellApp>(cfg: WindowConfig, app: A) -> Result<(), winit::error::EventLoopError> {
    install_system_clipboard();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut shell = Shell { cfg, app, gfx: None, list: DisplayList::new(), input: InputState::default() };
    event_loop.run_app(&mut shell)
}

struct Gfx {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    renderer: CpuRenderer,
}

struct Shell<A> {
    cfg: WindowConfig,
    app: A,
    gfx: Option<Gfx>,
    list: DisplayList,
    input: InputState,
}

impl<A: ShellApp> ApplicationHandler for Shell<A> {
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
            .with_inner_size(LogicalSize::new(self.cfg.size.0, self.cfg.size.1));
        let window = Rc::new(el.create_window(attrs).expect("create window"));
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&ctx, window.clone()).expect("softbuffer surface");
        let sz = window.inner_size();
        let mut renderer = CpuRenderer::new(sz.width.max(1), sz.height.max(1));
        renderer.clear_color = self.cfg.clear;
        renderer.fonts = self.app.fonts();
        window.set_ime_allowed(true);
        window.request_redraw();
        self.gfx = Some(Gfx { window, surface, renderer });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(g) = self.gfx.as_mut() else { return };
        let mut redraw = false;
        for ev in self.input.translate(&event) {
            redraw |= self.app.event(ev);
        }
        if redraw {
            g.window.request_redraw();
        }
        g.window.set_cursor(cursor_icon(self.app.cursor()));
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => g.window.request_redraw(),
            WindowEvent::RedrawRequested => {
                let sz = g.window.inner_size();
                let (Some(w), Some(h)) = (NonZeroU32::new(sz.width), NonZeroU32::new(sz.height)) else {
                    return; // minimized
                };
                if (g.renderer.fb.width, g.renderer.fb.height) != (sz.width, sz.height) {
                    g.renderer.resize(sz.width, sz.height);
                }
                let vp = Viewport { width: sz.width, height: sz.height, scale: g.window.scale_factor() };
                self.list.clear();
                self.app.draw(vp, &mut self.list);
                g.renderer.render(&self.list, vp);

                g.surface.resize(w, h).expect("resize surface");
                let mut buf = g.surface.buffer_mut().expect("buffer");
                for (dst, px) in buf.iter_mut().zip(g.renderer.fb.data.chunks_exact(4)) {
                    *dst = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
                }
                buf.present().expect("present");
            }
            _ => {}
        }
    }
}
