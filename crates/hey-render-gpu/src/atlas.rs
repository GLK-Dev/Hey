// Copyright (c) Vitaly Golik (mjojo)

//! Glyph atlas: one R8 page, shelf packing, cache keyed by (font, glyph, size).
//! When the page is full the atlas is flushed at the start of the next frame
//! (LRU eviction and extra pages are a later refinement).

use std::collections::HashMap;

pub const PAGE: u32 = 1024;

/// Position of a glyph bitmap inside the page, in texels.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
}

#[derive(Default)]
pub struct Atlas {
    slots: HashMap<(u32, u16, u32), Option<Slot>>,
    // Shelf packer state.
    cur_x: u32,
    cur_y: u32,
    row_h: u32,
    /// Set when a glyph did not fit; the page is cleared before the next frame.
    pub overflowed: bool,
}

impl Atlas {
    pub fn get(&self, key: (u32, u16, u32)) -> Option<Option<Slot>> {
        self.slots.get(&key).copied()
    }

    /// Reserves `w x h` texels. `None` when the page is full (the glyph is skipped this frame).
    pub fn alloc(&mut self, key: (u32, u16, u32), w: u32, h: u32) -> Option<Slot> {
        if self.cur_x + w > PAGE {
            self.cur_x = 0;
            self.cur_y += self.row_h;
            self.row_h = 0;
        }
        if w > PAGE || self.cur_y + h > PAGE {
            self.overflowed = true;
            return None;
        }
        let slot = Slot { x: self.cur_x, y: self.cur_y };
        self.cur_x += w;
        self.row_h = self.row_h.max(h);
        self.slots.insert(key, Some(slot));
        Some(slot)
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shelves_wrap_and_overflow() {
        let mut a = Atlas::default();
        let s0 = a.alloc((0, 0, 0), 600, 10).unwrap();
        let s1 = a.alloc((0, 1, 0), 600, 20).unwrap();
        assert_eq!((s0.x, s0.y), (0, 0));
        assert_eq!((s1.x, s1.y), (0, 10), "second glyph wraps to a new shelf");
        assert!(a.alloc((0, 2, 0), 10, PAGE).is_none());
        assert!(a.overflowed);
        a.clear();
        assert!(a.is_empty() && !a.overflowed);
    }
}
