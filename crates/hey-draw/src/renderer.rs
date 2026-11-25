// Copyright (c) Vitaly Golik (mjojo)

use crate::list::DisplayList;

/// Output area in physical pixels and scale (informational; commands are already in physical px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// Common backend contract (`CpuRenderer` is the reference, `GpuRenderer` is the fast path).
pub trait UiRenderer {
    fn resize(&mut self, width: u32, height: u32);
    fn render(&mut self, list: &DisplayList, viewport: Viewport);
}
