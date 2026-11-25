// Copyright (c) Vitaly Golik (mjojo)

//! Translation of winit events into normalized `UiEvent`s (physical pixels).

use hey_core::{Modifiers, MouseButton, Point, UiEvent};
use winit::{
    event::{ElementState, Ime, MouseScrollDelta, WindowEvent},
    keyboard::{Key, ModifiersState, NamedKey},
};

/// Scroll pixels per mouse wheel "line".
pub const LINE_PX: f32 = 40.0;

/// Input state between events: cursor position and modifiers.
#[derive(Default)]
pub struct InputState {
    pub pos: Point,
    pub mods: Modifiers,
}

pub fn mods(s: ModifiersState) -> Modifiers {
    Modifiers { shift: s.shift_key(), ctrl: s.control_key(), alt: s.alt_key(), logo: s.super_key() }
}

/// Logical key name: "Enter", "ArrowLeft", "a" (letters are lowercase).
pub fn key_name(k: &Key) -> Option<String> {
    match k {
        Key::Named(n) => Some(match n {
            NamedKey::Space => " ".to_string(),
            other => format!("{other:?}"),
        }),
        Key::Character(c) => Some(c.to_lowercase()),
        _ => None,
    }
}

fn button(b: winit::event::MouseButton) -> Option<MouseButton> {
    match b {
        winit::event::MouseButton::Left => Some(MouseButton::Left),
        winit::event::MouseButton::Right => Some(MouseButton::Right),
        winit::event::MouseButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

/// Printable text for `UiEvent::Text`: no control characters and no Ctrl/Alt/Logo combinations.
pub fn printable(text: &str, m: Modifiers) -> Option<String> {
    let ok = !text.is_empty() && !m.ctrl && !m.alt && !m.logo && text.chars().all(|c| !c.is_control());
    ok.then(|| text.to_string())
}

impl InputState {
    pub fn translate(&mut self, ev: &WindowEvent) -> Vec<UiEvent> {
        match ev {
            WindowEvent::CursorMoved { position, .. } => {
                self.pos = Point::new(position.x as f32, position.y as f32);
                vec![UiEvent::PointerMove { pos: self.pos }]
            }
            WindowEvent::CursorLeft { .. } => vec![UiEvent::PointerLeave],
            WindowEvent::MouseInput { state, button: b, .. } => {
                let Some(button) = button(*b) else { return vec![] };
                let pos = self.pos;
                vec![match state {
                    ElementState::Pressed => UiEvent::PointerDown { pos, button },
                    ElementState::Released => UiEvent::PointerUp { pos, button },
                }]
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * LINE_PX, y * LINE_PX),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                vec![UiEvent::Wheel { pos: self.pos, dx, dy }]
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = mods(m.state());
                vec![]
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let mut out = Vec::new();
                if let Some(code) = key_name(&event.logical_key) {
                    out.push(match event.state {
                        ElementState::Pressed => UiEvent::KeyDown { code, mods: self.mods },
                        ElementState::Released => UiEvent::KeyUp { code, mods: self.mods },
                    });
                }
                if event.state == ElementState::Pressed {
                    if let Some(t) = event.text.as_deref().and_then(|t| printable(t, self.mods)) {
                        out.push(UiEvent::Text(t));
                    }
                }
                out
            }
            WindowEvent::Ime(Ime::Commit(s)) => vec![UiEvent::Text(s.clone())],
            WindowEvent::Focused(f) => vec![UiEvent::FocusChanged(*f)],
            WindowEvent::Resized(s) => vec![UiEvent::Resize { width: s.width, height: s.height }],
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => vec![UiEvent::ScaleFactor(*scale_factor)],
            WindowEvent::HoveredFile(p) => vec![UiEvent::FileHovered(p.clone())],
            WindowEvent::DroppedFile(p) => vec![UiEvent::FileDropped(p.clone())],
            WindowEvent::HoveredFileCancelled => vec![UiEvent::FileHoverCancelled],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, WindowEvent};

    fn dev() -> DeviceId {
        DeviceId::dummy()
    }

    #[test]
    fn key_names() {
        assert_eq!(key_name(&Key::Named(NamedKey::Enter)).as_deref(), Some("Enter"));
        assert_eq!(key_name(&Key::Named(NamedKey::ArrowLeft)).as_deref(), Some("ArrowLeft"));
        assert_eq!(key_name(&Key::Named(NamedKey::Space)).as_deref(), Some(" "));
        assert_eq!(key_name(&Key::Character("A".into())).as_deref(), Some("a"));
        assert_eq!(key_name(&Key::Dead(None)), None);
    }

    #[test]
    fn printable_filters_controls_and_shortcuts() {
        let none = Modifiers::default();
        assert_eq!(printable("ж", none).as_deref(), Some("ж"));
        assert_eq!(printable("\r", none), None);
        assert_eq!(printable("", none), None);
        assert_eq!(printable("c", Modifiers { ctrl: true, ..none }), None);
        assert_eq!(printable("A", Modifiers { shift: true, ..none }).as_deref(), Some("A"));
    }

    #[test]
    fn pointer_events_carry_last_position() {
        let mut s = InputState::default();
        let ev = WindowEvent::CursorMoved { device_id: dev(), position: PhysicalPosition::new(12.0, 34.0) };
        assert_eq!(s.translate(&ev), [UiEvent::PointerMove { pos: Point::new(12.0, 34.0) }]);
        let down = WindowEvent::MouseInput { device_id: dev(), state: ElementState::Pressed, button: winit::event::MouseButton::Left };
        assert_eq!(s.translate(&down), [UiEvent::PointerDown { pos: Point::new(12.0, 34.0), button: MouseButton::Left }]);
        let back = WindowEvent::MouseInput { device_id: dev(), state: ElementState::Pressed, button: winit::event::MouseButton::Back };
        assert!(s.translate(&back).is_empty());
    }

    #[test]
    fn wheel_lines_become_pixels() {
        let mut s = InputState::default();
        let ev = WindowEvent::MouseWheel {
            device_id: dev(),
            delta: MouseScrollDelta::LineDelta(0.0, -2.0),
            phase: winit::event::TouchPhase::Moved,
        };
        assert_eq!(s.translate(&ev), [UiEvent::Wheel { pos: Point::default(), dx: 0.0, dy: -2.0 * LINE_PX }]);
    }
}
