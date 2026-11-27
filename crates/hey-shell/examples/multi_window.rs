// Copyright (c) Vitaly Golik (mjojo)

//! Click anywhere to open another window; close extra windows independently, close the first to quit.

use hey_core::{Color, Rect, UiEvent};
use hey_draw::{DisplayList, DrawCmd, Viewport};
use hey_shell::{run, ShellApp, WindowConfig, WindowKey, MAIN_WINDOW};

struct Demo {
    current: WindowKey,
    open: Vec<WindowKey>,
    next_key: WindowKey,
    requests: Vec<(WindowKey, WindowConfig)>,
}

impl ShellApp for Demo {
    fn focus_window(&mut self, key: WindowKey) {
        self.current = key;
    }

    fn draw(&mut self, vp: Viewport, list: &mut DisplayList) {
        // Every window gets its own tint so the routing is visible.
        let hue = (self.current * 70 % 200) as u8;
        let s = vp.scale as f32;
        list.push(DrawCmd::Rect {
            rect: Rect::new(40.0 * s, 40.0 * s, 200.0 * s, 120.0 * s),
            fill: Color::rgb(40 + hue, 90, 220 - hue),
            border: None,
            chamfer: 12.0 * s,
            shadow: None,
        });
    }

    fn event(&mut self, ev: UiEvent) -> bool {
        if matches!(ev, UiEvent::PointerDown { .. }) {
            let key = self.next_key;
            self.next_key += 1;
            self.open.push(key);
            let cfg = WindowConfig { title: format!("Hey window {key}"), size: (420.0, 300.0), ..WindowConfig::default() };
            self.requests.push((key, cfg));
        }
        false
    }

    fn take_new_windows(&mut self) -> Vec<(WindowKey, WindowConfig)> {
        std::mem::take(&mut self.requests)
    }

    fn window_closed(&mut self, key: WindowKey) {
        self.open.retain(|k| *k != key);
        eprintln!("closed window {key}, {} extra left", self.open.len());
    }
}

fn main() {
    let app = Demo { current: MAIN_WINDOW, open: Vec::new(), next_key: 1, requests: Vec::new() };
    run(WindowConfig { title: "Hey multi-window".into(), ..WindowConfig::default() }, app).unwrap();
}
