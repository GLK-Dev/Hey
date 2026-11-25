use hey_core::{Color, Point, Rect};
use hey_draw::{Border, DisplayList, DrawCmd, Shadow, Viewport};
use hey_shell::{run, ShellApp, WindowConfig};

struct Demo;

impl ShellApp for Demo {
    fn draw(&mut self, vp: Viewport, list: &mut DisplayList) {
        let s = vp.scale as f32;
        let r = Rect::new(60.0 * s, 60.0 * s, 360.0 * s, 200.0 * s);
        list.push(DrawCmd::Rect {
            rect: r,
            fill: Color::hex(0x1B2030),
            border: Some(Border { width: 2.0 * s, color: Color::hex(0x66CCFF) }),
            chamfer: 18.0 * s,
            shadow: Some(Shadow { offset: Point::new(0.0, 6.0 * s), blur: 24.0 * s, color: Color::rgba(0, 0, 0, 180) }),
        });
    }
}

fn main() {
    run(WindowConfig::default(), Demo).unwrap();
}
