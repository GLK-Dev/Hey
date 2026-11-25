// Copyright (c) Vitaly Golik (mjojo)

//! Image registry shared by the renderers: `ImageId` -> straight-alpha RGBA8 pixels.
//! The store is append-only, so an id never changes meaning once handed out.

use crate::ImageId;

/// Decoded bitmap, straight (non-premultiplied) alpha, row-major, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct ImageStore {
    images: Vec<Image>,
}

impl ImageStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a bitmap. `None` when it is empty or `rgba.len() != width * height * 4`.
    pub fn add(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> Option<ImageId> {
        if width == 0 || height == 0 || rgba.len() as u64 != width as u64 * height as u64 * 4 {
            return None;
        }
        self.images.push(Image { width, height, rgba });
        Some(ImageId(self.images.len() as u32 - 1))
    }

    pub fn get(&self, id: ImageId) -> Option<&Image> {
        self.images.get(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_validates_size_and_ids_are_stable() {
        let mut s = ImageStore::new();
        assert!(s.add(0, 1, vec![]).is_none());
        assert!(s.add(2, 1, vec![0; 7]).is_none());
        let a = s.add(1, 1, vec![1, 2, 3, 4]).unwrap();
        let b = s.add(2, 1, vec![0; 8]).unwrap();
        assert_ne!(a, b);
        assert_eq!(s.get(a).unwrap().rgba, [1, 2, 3, 4]);
        assert_eq!(s.get(b).unwrap().width, 2);
        assert!(s.get(ImageId(9)).is_none());
        assert_eq!(s.len(), 2);
    }
}
