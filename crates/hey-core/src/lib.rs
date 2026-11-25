//! hey-core: базовые типы без зависимостей от ОС и GPU.
//! Координаты — физические пиксели (ADR-3).

pub mod arena;
pub mod color;
pub mod event;
pub mod geom;

pub use arena::{Arena, NodeId};
pub use color::Color;
pub use event::{Key, Modifiers, MouseButton, UiEvent};
pub use geom::{Insets, Point, Rect, Size};
