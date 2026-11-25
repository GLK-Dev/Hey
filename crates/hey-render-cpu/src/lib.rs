//! hey-render-cpu: эталонный программный рендерер. Работает без окна (headless).
//!
//! Соглашения: центр пикселя `(i+0.5, j+0.5)`; рамка центрируется на границе
//! прямоугольника; отсечение округляется до целых пикселей; смешивание — straight alpha «over».
//! Текст рисуется из растров `hey_text::Fonts` (целые пиксели, без субпиксельного позиционирования).
//! Изображения пока пропускаются.

mod raster;

use std::sync::Arc;
use hey_text::Fonts;

use hey_core::Color;
use hey_draw::{DisplayList, UiRenderer, Viewport};

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// RGBA8, построчно сверху вниз.
    pub data: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, data: vec![0; (width * height * 4) as usize] }
    }

    pub fn clear(&mut self, c: Color) {
        for px in self.data.chunks_exact_mut(4) {
            px.copy_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> Color {
        let i = ((y * self.width + x) * 4) as usize;
        Color::rgba(self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3])
    }
}

pub struct CpuRenderer {
    pub fb: Framebuffer,
    pub clear_color: Color,
    /// Реестр шрифтов; без него `DrawCmd::Text` пропускается.
    pub fonts: Option<Arc<Fonts>>,
}

impl CpuRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { fb: Framebuffer::new(width, height), clear_color: Color::rgb(0, 0, 0), fonts: None }
    }
}

impl UiRenderer for CpuRenderer {
    fn resize(&mut self, width: u32, height: u32) {
        self.fb = Framebuffer::new(width, height);
    }

    fn render(&mut self, list: &DisplayList, _viewport: Viewport) {
        debug_assert!(list.validate().is_ok(), "некорректный DisplayList");
        self.fb.clear(self.clear_color);
        for (cmd, clip) in list.iter_clipped() {
            raster::draw(&mut self.fb, self.fonts.as_deref(), cmd, clip);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hey_core::{Point, Rect};
    use hey_draw::{Border, DrawCmd};

    fn render(cmds: Vec<DrawCmd>, w: u32, h: u32) -> Framebuffer {
        let mut r = CpuRenderer::new(w, h);
        let mut l = DisplayList::new();
        cmds.into_iter().for_each(|c| l.push(c));
        r.render(&l, Viewport { width: w, height: h, scale: 1.0 });
        r.fb
    }

    const RED: Color = Color::rgb(255, 0, 0);

    #[test]
    fn solid_rect_is_pixel_exact() {
        let fb = render(vec![DrawCmd::fill(Rect::new(2.0, 2.0, 4.0, 4.0), RED)], 8, 8);
        assert_eq!(fb.pixel(2, 2), RED);
        assert_eq!(fb.pixel(5, 5), RED);
        assert_eq!(fb.pixel(6, 6), Color::rgb(0, 0, 0));
        assert_eq!(fb.pixel(1, 3), Color::rgb(0, 0, 0));
    }

    #[test]
    fn chamfer_cuts_top_left_and_bottom_right_only() {
        let cmd = DrawCmd::Rect {
            rect: Rect::new(0.0, 0.0, 16.0, 16.0),
            fill: RED,
            border: None,
            chamfer: 8.0,
            shadow: None,
        };
        let fb = render(vec![cmd], 16, 16);
        let bg = Color::rgb(0, 0, 0);
        assert_eq!(fb.pixel(0, 0), bg, "TL срезан");
        assert_eq!(fb.pixel(15, 15), bg, "BR срезан");
        assert_eq!(fb.pixel(15, 0), RED, "TR цел");
        assert_eq!(fb.pixel(0, 15), RED, "BL цел");
        assert_eq!(fb.pixel(8, 8), RED);
    }

    #[test]
    fn clip_limits_drawing() {
        let fb = render(
            vec![
                DrawCmd::ClipPush(Rect::new(0.0, 0.0, 4.0, 8.0)),
                DrawCmd::fill(Rect::new(0.0, 0.0, 8.0, 8.0), RED),
                DrawCmd::ClipPop,
            ],
            8,
            8,
        );
        assert_eq!(fb.pixel(3, 4), RED);
        assert_eq!(fb.pixel(4, 4), Color::rgb(0, 0, 0));
    }

    #[test]
    fn border_is_centered_on_edge() {
        let cmd = DrawCmd::Rect {
            rect: Rect::new(4.0, 4.0, 8.0, 8.0),
            fill: Color::rgb(0, 0, 255),
            border: Some(Border { width: 2.0, color: RED }),
            chamfer: 0.0,
            shadow: None,
        };
        let fb = render(vec![cmd], 16, 16);
        assert_eq!(fb.pixel(3, 8), RED, "наружная половина рамки");
        assert_eq!(fb.pixel(4, 8), RED, "внутренняя половина рамки");
        assert_eq!(fb.pixel(5, 8), Color::rgb(0, 0, 255));
        assert_eq!(fb.pixel(2, 8), Color::rgb(0, 0, 0));
    }

    #[test]
    fn line_covers_its_pixels() {
        let fb = render(
            vec![DrawCmd::Line {
                a: Point::new(0.0, 4.0),
                b: Point::new(8.0, 4.0),
                width: 2.0,
                color: RED,
            }],
            8,
            8,
        );
        assert_eq!(fb.pixel(4, 3), RED);
        assert_eq!(fb.pixel(4, 4), RED);
        assert_eq!(fb.pixel(4, 6), Color::rgb(0, 0, 0));
    }
}

impl Framebuffer {
    /// Кодирует кадр в PNG (RGBA8).
    pub fn encode_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        w.write_image_data(&self.data).expect("png data");
        w.finish().expect("png finish");
        out
    }

    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.encode_png())
    }

    /// Декодирует PNG (RGBA8) в кадр.
    pub fn decode_png(bytes: &[u8]) -> Result<Self, png::DecodingError> {
        let mut r = png::Decoder::new(bytes).read_info()?;
        let mut buf = vec![0; r.output_buffer_size()];
        let info = r.next_frame(&mut buf)?;
        buf.truncate(info.buffer_size());
        Ok(Self { width: info.width, height: info.height, data: buf })
    }
}
