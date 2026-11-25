//! hey-layout: flex-раскладка (Row/Column/Stack/Spacer), двухпроходная: measure → arrange.
//!
//! Все размеры — физические пиксели (ADR-3). Итоговые прямоугольники абсолютные
//! (в координатах корня) и притянуты к целым пикселям.

mod style;
mod tree;

pub use style::{Align, Axis, Dim, Justify, Kind, Style};
pub use tree::LayoutTree;
