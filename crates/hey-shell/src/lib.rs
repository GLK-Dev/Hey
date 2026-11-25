// Copyright (c) Vitaly Golik (mjojo)

//! hey-shell: окно (winit) и вывод кадра `CpuRenderer` через softbuffer.
//!
//! Все размеры в физических пикселях (ADR-3); `scale` передаётся приложению для справки.

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

/// Приложение для оболочки: строит display list на каждый кадр.
pub trait ShellApp {
    fn draw(&mut self, viewport: Viewport, list: &mut DisplayList);

    /// Реестр шрифтов для отрисовки `DrawCmd::Text`; вызывается при создании окна.
    fn fonts(&self) -> Option<std::sync::Arc<hey_text::Fonts>> {
        None
    }
}

pub struct WindowConfig {
    pub title: String,
    /// Логический размер при создании.
    pub size: (f64, f64),
    pub clear: hey_core::Color,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: "Hey".into(), size: (960.0, 600.0), clear: hey_core::Color::hex(0x0B0C10) }
    }
}

pub fn run<A: ShellApp>(cfg: WindowConfig, app: A) -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut shell = Shell { cfg, app, gfx: None, list: DisplayList::new() };
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
}

impl<A: ShellApp> ApplicationHandler for Shell<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.cfg.title.clone())
            .with_inner_size(LogicalSize::new(self.cfg.size.0, self.cfg.size.1));
        let window = Rc::new(el.create_window(attrs).expect("создание окна"));
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&ctx, window.clone()).expect("softbuffer surface");
        let sz = window.inner_size();
        let mut renderer = CpuRenderer::new(sz.width.max(1), sz.height.max(1));
        renderer.clear_color = self.cfg.clear;
        renderer.fonts = self.app.fonts();
        window.request_redraw();
        self.gfx = Some(Gfx { window, surface, renderer });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(g) = self.gfx.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => g.window.request_redraw(),
            WindowEvent::RedrawRequested => {
                let sz = g.window.inner_size();
                let (Some(w), Some(h)) = (NonZeroU32::new(sz.width), NonZeroU32::new(sz.height)) else {
                    return; // свёрнуто
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
