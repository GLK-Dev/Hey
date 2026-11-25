// Copyright (c) Vitaly Golik (mjojo)

use crate::geom::Point;

/// Node identifier for diffing between frames (local state is carried over by key).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Index(u64),
    Name(String),
}

impl From<u64> for Key {
    fn from(v: u64) -> Self {
        Key::Index(v)
    }
}

impl From<&str> for Key {
    fn from(v: &str) -> Self {
        Key::Name(v.to_owned())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Normalized events independent of winit (positions are in physical pixels).
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    PointerMove {
        pos: Point,
    },
    PointerDown {
        pos: Point,
        button: MouseButton,
    },
    PointerUp {
        pos: Point,
        button: MouseButton,
    },
    PointerLeave,
    Wheel {
        pos: Point,
        dx: f32,
        dy: f32,
    },
    /// `code` is the logical key name ("Enter", "ArrowLeft", "a").
    KeyDown {
        code: String,
        mods: Modifiers,
    },
    KeyUp {
        code: String,
        mods: Modifiers,
    },
    /// Entered text (already accounting for keyboard layout/IME).
    Text(String),
    FocusChanged(bool),
    Resize {
        width: u32,
        height: u32,
    },
    ScaleFactor(f64),
    /// A file is hovering over the window (drag-and-drop), not yet dropped.
    FileHovered(std::path::PathBuf),
    FileDropped(std::path::PathBuf),
    FileHoverCancelled,
}

/// Mouse cursor shape over an element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorIcon {
    #[default]
    Default,
    /// Clickable element.
    Pointer,
    /// Editable text.
    Text,
    NotAllowed,
    ResizeH,
    ResizeV,
}
