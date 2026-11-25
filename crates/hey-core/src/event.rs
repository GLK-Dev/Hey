// Copyright (c) Vitaly Golik (mjojo)

use crate::geom::Point;

/// Идентификатор узла для диффинга между кадрами (локальное состояние переносится по ключу).
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

/// Нормализованные события, не зависящие от winit (позиции — в физических пикселях).
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    PointerMove { pos: Point },
    PointerDown { pos: Point, button: MouseButton },
    PointerUp { pos: Point, button: MouseButton },
    PointerLeave,
    Wheel { pos: Point, dx: f32, dy: f32 },
    /// `code` — логическое имя клавиши («Enter», «ArrowLeft», «a»).
    KeyDown { code: String, mods: Modifiers },
    KeyUp { code: String, mods: Modifiers },
    /// Введённый текст (уже с учётом раскладки/IME).
    Text(String),
    FocusChanged(bool),
    Resize { width: u32, height: u32 },
    ScaleFactor(f64),
    /// Файл наведён на окно (drag-and-drop), ещё не отпущен.
    FileHovered(std::path::PathBuf),
    FileDropped(std::path::PathBuf),
    FileHoverCancelled,
}

/// Вид курсора мыши над элементом.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorIcon {
    #[default]
    Default,
    /// Кликабельный элемент.
    Pointer,
    /// Редактируемый текст.
    Text,
    NotAllowed,
    ResizeH,
    ResizeV,
}
