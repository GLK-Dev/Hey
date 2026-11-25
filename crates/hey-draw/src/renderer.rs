use crate::list::DisplayList;

/// Область вывода в физических пикселях и масштаб (для справки; команды уже в физических px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// Общий контракт бэкендов (`CpuRenderer` — эталон, `GpuRenderer` — быстрый путь).
pub trait UiRenderer {
    fn resize(&mut self, width: u32, height: u32);
    fn render(&mut self, list: &DisplayList, viewport: Viewport);
}
