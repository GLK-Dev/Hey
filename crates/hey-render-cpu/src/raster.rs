// Copyright (c) Vitaly Golik (mjojo)

use crate::Framebuffer;
use hey_core::{Color, Point, Rect};
use hey_draw::{Border, DrawCmd, GlyphRun, ImageStore, Shadow};
use hey_text::Fonts;

const SQRT2: f32 = std::f32::consts::SQRT_2;

pub fn draw(fb: &mut Framebuffer, fonts: Option<&Fonts>, images: Option<&ImageStore>, cmd: &DrawCmd, clip: Option<Rect>) {
    match cmd {
        DrawCmd::Rect { rect, fill, border, chamfer, shadow } => {
            if let Some(s) = shadow {
                draw_shadow(fb, rect, s, clip);
            }
            let c = chamfer.max(0.0);
            paint(fb, rect.inflate(1.0), clip, |x, y| (*fill, shape_cov(rect, c, x, y)));
            if let Some(Border { width, color }) = border {
                let h = width / 2.0;
                let outer = (rect.inflate(h), c + h * (2.0 - SQRT2));
                let inner = (rect.inflate(-h), (c - h * (2.0 - SQRT2)).max(0.0));
                paint(fb, rect.inflate(h), clip, |x, y| {
                    let a = shape_cov(&outer.0, outer.1, x, y) - shape_cov(&inner.0, inner.1, x, y);
                    (*color, a.max(0.0))
                });
            }
        }
        DrawCmd::Line { a, b, width, color } => {
            let h = width / 2.0;
            let (x0, y0) = (a.x.min(b.x) - h - 1.0, a.y.min(b.y) - h - 1.0);
            let bb = Rect::new(x0, y0, a.x.max(b.x) + h + 1.0 - x0, a.y.max(b.y) + h + 1.0 - y0);
            paint(fb, bb, clip, |x, y| {
                let d = dist_to_segment(Point::new(x + 0.5, y + 0.5), *a, *b);
                (*color, (0.5 + h - d).clamp(0.0, 1.0))
            });
        }
        DrawCmd::Text(run) => {
            if let Some(fonts) = fonts {
                draw_text(fb, fonts, run, clip);
            }
        }
        DrawCmd::Image { rect, image } => {
            if let Some(img) = images.and_then(|s| s.get(*image)) {
                draw_image(fb, img, rect, clip);
            }
        }
        DrawCmd::ClipPush(_) | DrawCmd::ClipPop => {}
        _ => {}
    }
}

/// Glyphs are placed on whole pixels (the pen is rounded); coverage comes from the font raster.
fn draw_text(fb: &mut Framebuffer, fonts: &Fonts, run: &GlyphRun, clip: Option<Rect>) {
    for g in &run.glyphs {
        let bmp = fonts.bitmap(run.font, g.id as u16, run.size_px);
        if bmp.w == 0 || bmp.h == 0 {
            continue;
        }
        let x0 = g.x.round() as i32 + bmp.xmin;
        let y0 = g.y.round() as i32 - bmp.ymin - bmp.h as i32;
        let area = Rect::new(x0 as f32, y0 as f32, bmp.w as f32, bmp.h as f32);
        paint(fb, area, clip, |x, y| {
            let (dx, dy) = (x as i32 - x0, y as i32 - y0);
            let cov = bmp.data[dy as usize * bmp.w as usize + dx as usize] as f32 / 255.0;
            (run.color, cov)
        });
    }
}

/// The rectangle is snapped to whole pixels and the bitmap is sampled nearest-neighbour
/// with integer math (`floor((x - x0) * src_w / dst_w)`), so the GPU can match it exactly.
fn draw_image(fb: &mut Framebuffer, img: &hey_draw::Image, rect: &Rect, clip: Option<Rect>) {
    let (x0, y0) = (rect.x.round() as i64, rect.y.round() as i64);
    let (w, h) = ((rect.right().round() as i64 - x0).max(0), (rect.bottom().round() as i64 - y0).max(0));
    if w == 0 || h == 0 {
        return;
    }
    let area = Rect::new(x0 as f32, y0 as f32, w as f32, h as f32);
    paint(fb, area, clip, |x, y| {
        let sx = ((x as i64 - x0) * img.width as i64 / w) as usize;
        let sy = ((y as i64 - y0) * img.height as i64 / h) as usize;
        let i = (sy * img.width as usize + sx) * 4;
        (Color::rgba(img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]), 1.0)
    });
}

trait RectExt {
    fn inflate(&self, d: f32) -> Rect;
}
impl RectExt for Rect {
    fn inflate(&self, d: f32) -> Rect {
        Rect::new(self.x - d, self.y - d, (self.w + 2.0 * d).max(0.0), (self.h + 2.0 * d).max(0.0))
    }
}

/// Overlap of pixel `[p, p+1]` with segment `[a, b]`.
fn overlap(p: f32, a: f32, b: f32) -> f32 {
    ((p + 1.0).min(b) - p.max(a)).clamp(0.0, 1.0)
}

/// Coverage of pixel `(px,py)` (top-left corner) by a "rectangle with TL/BR chamfer" shape.
fn shape_cov(r: &Rect, c: f32, px: f32, py: f32) -> f32 {
    let mut cov = overlap(px, r.x, r.right()) * overlap(py, r.y, r.bottom());
    if c > 0.0 {
        let (cx, cy) = (px + 0.5, py + 0.5);
        let tl = ((cx + cy) - (r.x + r.y + c)) / SQRT2;
        let br = ((r.right() + r.bottom() - c) - (cx + cy)) / SQRT2;
        cov *= (0.5 + tl).clamp(0.0, 1.0) * (0.5 + br).clamp(0.0, 1.0);
    }
    cov
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0) };
    let (qx, qy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - qx).powi(2) + (p.y - qy).powi(2)).sqrt()
}

/// erf approximation (Abramowitz–Stegun 7.1.26).
fn erf(x: f32) -> f32 {
    let s = x.signum();
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0 - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_72) * t + 0.254_829_6) * t * (-x * x).exp();
    s * y
}

fn draw_shadow(fb: &mut Framebuffer, rect: &Rect, s: &Shadow, clip: Option<Rect>) {
    let r = rect.translate(s.offset.x, s.offset.y);
    let sigma = (s.blur / 2.0).max(0.01);
    let reach = s.blur * 1.5 + 1.0;
    let k = sigma * SQRT2;
    let axis = |p: f32, a: f32, b: f32| 0.5 * (erf((b - p) / k) - erf((a - p) / k));
    paint(fb, r.inflate(reach), clip, |x, y| {
        let (cx, cy) = (x + 0.5, y + 0.5);
        (s.color, axis(cx, r.x, r.right()) * axis(cy, r.y, r.bottom()))
    });
}

/// Walks pixels of `area ∩ clip ∩ framebuffer`; `f(x, y)` -> (color, coverage).
fn paint(fb: &mut Framebuffer, area: Rect, clip: Option<Rect>, f: impl Fn(f32, f32) -> (Color, f32)) {
    let mut x0 = area.x.floor().max(0.0) as i64;
    let mut y0 = area.y.floor().max(0.0) as i64;
    let mut x1 = (area.right().ceil() as i64).min(fb.width as i64);
    let mut y1 = (area.bottom().ceil() as i64).min(fb.height as i64);
    if let Some(c) = clip {
        x0 = x0.max(c.x.round() as i64);
        y0 = y0.max(c.y.round() as i64);
        x1 = x1.min(c.right().round() as i64);
        y1 = y1.min(c.bottom().round() as i64);
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let (color, cov) = f(x as f32, y as f32);
            blend(fb, x as u32, y as u32, color, cov);
        }
    }
}

fn blend(fb: &mut Framebuffer, x: u32, y: u32, c: Color, cov: f32) {
    let sa = (c.a as f32 / 255.0) * cov.clamp(0.0, 1.0);
    if sa <= 0.0 {
        return;
    }
    let i = ((y * fb.width + x) * 4) as usize;
    let da = fb.data[i + 3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    let ch = |s: u8, d: u8| {
        let v = (s as f32 * sa + d as f32 * da * (1.0 - sa)) / oa;
        v.round().clamp(0.0, 255.0) as u8
    };
    fb.data[i] = ch(c.r, fb.data[i]);
    fb.data[i + 1] = ch(c.g, fb.data[i + 1]);
    fb.data[i + 2] = ch(c.b, fb.data[i + 2]);
    fb.data[i + 3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
}
