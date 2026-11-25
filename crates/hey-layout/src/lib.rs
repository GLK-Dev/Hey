//! hey-layout: flex layout (Row/Column/Stack/Spacer), two passes: measure → arrange.
//!
//! All sizes are physical pixels (ADR-3). Resulting rectangles are absolute
//! (in root coordinates) and snapped to whole pixels.

mod style;
mod tree;

pub use style::{Align, Axis, Dim, Justify, Kind, Style};
pub use tree::LayoutTree;
