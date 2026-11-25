// Copyright (c) Vitaly Golik (mjojo)

//! Local widget state by `Key` (scroll, caret); survives tree rebuilds.

use crate::event::Key;
use std::any::Any;
use std::collections::HashMap;

struct Slot {
    value: Box<dyn Any>,
    touched: bool,
}

#[derive(Default)]
pub struct WidgetStateStore {
    map: HashMap<Key, Slot>,
}

impl WidgetStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// State by key; created via `Default`. If a different type is stored under the key, it is replaced.
    pub fn get_or_default<T: Any + Default>(&mut self, key: &Key) -> &mut T {
        let slot = self.map.entry(key.clone()).or_insert_with(|| Slot { value: Box::new(T::default()), touched: true });
        slot.touched = true;
        if !slot.value.is::<T>() {
            slot.value = Box::new(T::default());
        }
        slot.value.downcast_mut::<T>().expect("type checked above")
    }

    pub fn get<T: Any>(&self, key: &Key) -> Option<&T> {
        self.map.get(key)?.value.downcast_ref()
    }

    pub fn remove(&mut self, key: &Key) -> bool {
        self.map.remove(key).is_some()
    }

    /// Frame start: resets the usage marks.
    pub fn begin_frame(&mut self) {
        self.map.values_mut().for_each(|s| s.touched = false);
    }

    /// Frame end: drops the state of keys not requested during this frame.
    pub fn end_frame(&mut self) {
        self.map.retain(|_, s| s.touched);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Debug, PartialEq)]
    struct Scroll(f32);

    #[test]
    fn persists_between_frames() {
        let mut s = WidgetStateStore::new();
        let k = Key::from("list");
        s.begin_frame();
        s.get_or_default::<Scroll>(&k).0 = 42.0;
        s.end_frame();
        s.begin_frame();
        assert_eq!(s.get_or_default::<Scroll>(&k).0, 42.0);
        s.end_frame();
    }

    #[test]
    fn untouched_keys_are_swept() {
        let mut s = WidgetStateStore::new();
        s.begin_frame();
        s.get_or_default::<Scroll>(&Key::from("a"));
        s.end_frame();
        s.begin_frame();
        s.end_frame();
        assert!(s.is_empty());
    }

    #[test]
    fn type_mismatch_resets() {
        let mut s = WidgetStateStore::new();
        let k = Key::from(1u64);
        s.get_or_default::<Scroll>(&k).0 = 5.0;
        assert_eq!(*s.get_or_default::<u32>(&k), 0);
        assert!(s.get::<Scroll>(&k).is_none());
    }
}
