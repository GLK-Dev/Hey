// Copyright (c) Vitaly Golik (mjojo)

//! hey-core: base types with no OS or GPU dependencies.
//! Coordinates are physical pixels (ADR-3).

pub mod arena;
pub mod color;
pub mod event;
pub mod geom;
pub mod msg;
pub mod state;
pub mod tree;

pub use arena::{Arena, NodeId};
pub use color::Color;
pub use event::{CursorIcon, Key, Modifiers, MouseButton, UiEvent};
pub use geom::{Insets, Point, Rect, Size};
pub use msg::{Callback, MsgSink};
pub use state::WidgetStateStore;
pub use tree::Tree;
