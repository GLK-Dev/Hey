// Copyright (c) Vitaly Golik (mjojo)

//! SVG icon rasterization for the `ImageStore` (`resvg`, no text support needed).
//! Icons are rendered to a fixed physical pixel size (ADR-3), so re-rasterize on scale change.

use hey_core::Color;
use hey_draw::{ImageId, ImageStore};
use resvg::{tiny_skia, usvg};

/// Rasterizes `svg` to a `size x size` straight-alpha RGBA buffer, fitting the icon's viewbox
/// (aspect ratio preserved, centered). With `tint`, every pixel takes the tint's RGB and keeps its own alpha
/// (multiplied by the tint's alpha), which turns a colored icon into a theme-colored glyph.
pub fn rasterize(svg: &[u8], size: u32, tint: Option<Color>) -> Option<Vec<u8>> {
    if size == 0 {
        return None;
    }
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
    let ts = tree.size();
    if ts.width() <= 0.0 || ts.height() <= 0.0 {
        return None;
    }
    let s = size as f32;
    let k = (s / ts.width()).min(s / ts.height());
    let (dx, dy) = ((s - ts.width() * k) / 2.0, (s - ts.height() * k) / 2.0);
    let mut pm = tiny_skia::Pixmap::new(size, size)?;
    resvg::render(&tree, tiny_skia::Transform::from_row(k, 0.0, 0.0, k, dx, dy), &mut pm.as_mut());

    // tiny-skia stores premultiplied RGBA; the image store wants straight alpha.
    let mut out = Vec::with_capacity(pm.data().len());
    for p in pm.pixels() {
        let a = p.alpha();
        let (r, g, b) = match tint {
            Some(t) => (t.r, t.g, t.b),
            None if a == 0 => (0, 0, 0),
            None => {
                let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                (un(p.red()), un(p.green()), un(p.blue()))
            }
        };
        let a = match tint {
            Some(t) => ((a as u32 * t.a as u32 + 127) / 255) as u8,
            None => a,
        };
        out.extend_from_slice(&[r, g, b, a]);
    }
    Some(out)
}

/// Rasterizes `svg` and appends it to `store`. `None` if the SVG is invalid or `size` is 0.
pub fn add_svg(store: &mut ImageStore, svg: &[u8], size: u32, tint: Option<Color>) -> Option<ImageId> {
    let rgba = rasterize(svg, size, tint)?;
    store.add(size, size, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8" viewBox="0 0 8 8"><rect x="0" y="0" width="4" height="8" fill="#FF0000"/></svg>"##;

    fn px(buf: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn renders_straight_alpha_and_scales() {
        let b = rasterize(SQUARE, 16, None).unwrap();
        assert_eq!(b.len(), 16 * 16 * 4);
        assert_eq!(px(&b, 16, 2, 8), [255, 0, 0, 255]);
        assert_eq!(px(&b, 16, 12, 8)[3], 0, "right half is transparent");
    }

    #[test]
    fn tint_replaces_rgb_keeps_alpha() {
        let b = rasterize(SQUARE, 8, Some(Color::rgba(10, 20, 30, 128))).unwrap();
        assert_eq!(px(&b, 8, 1, 4), [10, 20, 30, 128]);
        assert_eq!(px(&b, 8, 6, 4)[3], 0);
    }

    #[test]
    fn rejects_garbage_and_zero_size() {
        assert!(rasterize(b"not svg", 16, None).is_none());
        assert!(rasterize(SQUARE, 0, None).is_none());
        let mut store = ImageStore::new();
        assert!(add_svg(&mut store, SQUARE, 16, None).is_some());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn real_design_icons_parse() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/icons/small");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "svg") {
                let data = std::fs::read(e.path()).unwrap();
                assert!(rasterize(&data, 16, None).is_some(), "{:?}", e.path());
                n += 1;
            }
        }
        assert!(n > 0);
    }
}
