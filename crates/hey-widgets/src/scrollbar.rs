//! Scrollbar geometry for layout scroll containers (`Style::scroll`).

use hey_core::{Point, Rect};

/// Scrollbar thickness in logical px.
pub(crate) const THICKNESS: f32 = 8.0;
const MARGIN: f32 = 2.0;
const MIN_THUMB: f32 = 24.0;

/// Track and thumb rectangles of one scrollbar.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bar {
    pub vertical: bool,
    pub track: Rect,
    pub thumb: Rect,
    /// Visible length along the scroll axis.
    pub view: f32,
}

impl Bar {
    fn start(&self, r: &Rect) -> f32 {
        if self.vertical {
            r.y
        } else {
            r.x
        }
    }

    fn len(&self, r: &Rect) -> f32 {
        if self.vertical {
            r.h
        } else {
            r.w
        }
    }

    /// Coordinate of `p` along the scroll axis.
    pub fn axis(&self, p: Point) -> f32 {
        if self.vertical {
            p.y
        } else {
            p.x
        }
    }

    /// Scroll offset that puts the thumb start at `pos - grab` (`max` is the maximum offset).
    pub fn offset_for(&self, pos: f32, grab: f32, max: f32) -> f32 {
        let free = self.len(&self.track) - self.len(&self.thumb);
        if free <= 0.0 {
            return 0.0;
        }
        ((pos - grab - self.start(&self.track)) / free * max).clamp(0.0, max)
    }

    /// Offset of the pointer inside the thumb along the axis.
    pub fn grab(&self, p: Point) -> f32 {
        self.axis(p) - self.start(&self.thumb)
    }
}

/// Scrollbar of a container. `view` is the visible content rectangle (the rect without padding),
/// `scroll` and `max` are the current and maximum offsets. `None` when nothing overflows.
pub(crate) fn bar(rect: Rect, view: Rect, scroll: Point, max: Point, vertical: bool, scale: f32) -> Option<Bar> {
    let (max, scroll, view_len) = if vertical { (max.y, scroll.y, view.h) } else { (max.x, scroll.x, view.w) };
    if max <= 0.0 {
        return None;
    }
    let t = THICKNESS * scale;
    let m = MARGIN * scale;
    let track = if vertical {
        Rect::new(rect.x + rect.w - t - m, rect.y + m, t, rect.h - 2.0 * m)
    } else {
        Rect::new(rect.x + m, rect.y + rect.h - t - m, rect.w - 2.0 * m, t)
    };
    let track_len = if vertical { track.h } else { track.w };
    if track_len <= 0.0 {
        return None;
    }
    let content = view_len + max;
    let thumb_len = (track_len * view_len / content).max(MIN_THUMB * scale).min(track_len);
    let pos = (track_len - thumb_len) * (scroll / max).clamp(0.0, 1.0);
    let thumb = if vertical { Rect::new(track.x, track.y + pos, t, thumb_len) } else { Rect::new(track.x + pos, track.y, thumb_len, t) };
    Some(Bar { vertical, track, thumb, view: view_len })
}
