// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{Callback, Modifiers, MouseButton, MsgSink, NodeId, Point, Rect, Size, UiEvent};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::{LayoutTree, Style};
use hey_style::{Theme, WidgetState};
use hey_text::{FontId, Fonts};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Paint/measure context shared by all widgets.
pub struct Env {
    pub theme: Theme,
    pub fonts: std::sync::Arc<Fonts>,
    pub chain: Vec<FontId>,
    /// Base font size (physical px).
    pub font_px: f32,
}

/// Widget behavior. State (hover/pressed/focus/disabled) is kept by `Ui`.
pub trait Widget<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList);
    fn focusable(&self) -> bool {
        false
    }
    /// Mouse cursor when hovering over the widget.
    fn cursor(&self) -> hey_core::CursorIcon {
        hey_core::CursorIcon::Default
    }
    /// Mouse click or Enter/Space on a focused widget.
    fn activate(&mut self, _sink: &MsgSink<Msg>) {}
    /// Click on the widget (position in absolute coordinates). `activate` by default.
    fn click(&mut self, _env: &Env, _rect: Rect, _pos: Point, sink: &MsgSink<Msg>) {
        self.activate(sink);
    }
    /// Left button press on the widget; `clicks` is 1, 2, 3… (a series of quick presses).
    fn press(&mut self, _env: &Env, _rect: Rect, _pos: Point, _clicks: u32, _sink: &MsgSink<Msg>) {}
    /// Pointer motion with the button held (capture: delivered to the pressed widget even outside it).
    fn drag(&mut self, _env: &Env, _rect: Rect, _pos: Point, _sink: &MsgSink<Msg>) {}
    /// Mouse wheel (`dy` < 0 is down, in physical px); `true` means handled.
    fn wheel(&mut self, _env: &Env, _rect: Rect, _dx: f32, _dy: f32) -> bool {
        false
    }
    /// Key on the focused widget; `true` means handled.
    fn key(&mut self, _code: &str, _mods: Modifiers, _sink: &MsgSink<Msg>) -> bool {
        false
    }
    /// Printable text (keyboard/IME) on the focused widget; `true` means handled.
    /// Pointer motion over an overlay (for row highlighting). `true` means a redraw is needed.
    fn pointer_move(&mut self, _env: &Env, _rect: Rect, _pos: Point) -> bool {
        false
    }
    fn text(&mut self, _text: &str, _sink: &MsgSink<Msg>) -> bool {
        false
    }
    /// Local state (scroll, caret) that survives rebuilding the widget.
    fn save_state(&self) -> Option<Box<dyn Any>> {
        None
    }
    fn load_state(&mut self, _state: Box<dyn Any>) {}
}

/// Overlay kind: popup (closes on outside click), modal
/// (dimmed, blocks everything else) or hint (takes no input).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    Popup,
    Modal,
    Tooltip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayId(u32);

struct Overlay<Msg> {
    id: OverlayId,
    widget: Box<dyn Widget<Msg>>,
    rect: Rect,
    kind: OverlayKind,
    hover: bool,
    pressed: bool,
}

struct Queue<Msg> {
    next: u32,
    open: Vec<Overlay<Msg>>,
    /// `None` closes the topmost interactive overlay.
    close: Vec<Option<OverlayId>>,
    viewport: Size,
}

/// Shared overlay handle: widgets open/close overlays through it,
/// `Ui` applies the requests after handling an event.
pub struct Overlays<Msg>(Rc<RefCell<Queue<Msg>>>);

impl<Msg> Clone for Overlays<Msg> {
    fn clone(&self) -> Self {
        Overlays(self.0.clone())
    }
}

impl<Msg> Overlays<Msg> {
    pub fn open(&self, rect: Rect, widget: impl Widget<Msg> + 'static, kind: OverlayKind) -> OverlayId {
        let mut q = self.0.borrow_mut();
        let id = OverlayId(q.next);
        q.next += 1;
        q.open.push(Overlay { id, widget: Box::new(widget), rect, kind, hover: false, pressed: false });
        id
    }
    pub fn close(&self, id: OverlayId) {
        self.0.borrow_mut().close.push(Some(id));
    }
    pub fn close_top(&self) {
        self.0.borrow_mut().close.push(None);
    }
    /// Window size at the last paint.
    pub fn viewport(&self) -> Size {
        self.0.borrow().viewport
    }
}

pub(crate) struct Node<Msg> {
    pub(crate) widget: Box<dyn Widget<Msg>>,
    pub(crate) disabled: bool,
}

pub struct Ui<Msg> {
    pub tree: LayoutTree,
    pub env: Rc<Env>,
    sink: MsgSink<Msg>,
    pub(crate) nodes: HashMap<NodeId, Node<Msg>>,
    /// Key and property hash of nodes created via `reconcile`.
    pub(crate) keys: HashMap<NodeId, (hey_core::Key, u64)>,
    hover: Option<NodeId>,
    pressed: Option<NodeId>,
    focus: Option<NodeId>,
    trap: Option<NodeId>,
    last_click: Option<(NodeId, Instant, u32)>,
    overlays: Vec<Overlay<Msg>>,
    queue: Overlays<Msg>,
    file_drop: Option<Callback<Msg, std::path::PathBuf>>,
    file_hover: bool,
}

const MULTI_CLICK: Duration = Duration::from_millis(500);

impl<Msg: 'static> Ui<Msg> {
    pub fn new(theme: Theme, fonts: std::sync::Arc<Fonts>, chain: Vec<FontId>, sink: MsgSink<Msg>) -> Self {
        let font_px = theme.space(14.0);
        Ui {
            tree: LayoutTree::new(Style::column()),
            env: Rc::new(Env { theme, fonts, chain, font_px }),
            sink,
            nodes: HashMap::new(),
            keys: HashMap::new(),
            hover: None,
            pressed: None,
            focus: None,
            trap: None,
            last_click: None,
            overlays: Vec::new(),
            file_drop: None,
            file_hover: false,
            queue: Overlays(Rc::new(RefCell::new(Queue { next: 0, open: Vec::new(), close: Vec::new(), viewport: Size::new(0.0, 0.0) }))),
        }
    }

    /// Handle for opening overlays from widgets.
    pub fn overlays(&self) -> Overlays<Msg> {
        self.queue.clone()
    }

    pub fn open_overlay(&mut self, rect: Rect, widget: impl Widget<Msg> + 'static, kind: OverlayKind) -> OverlayId {
        let id = self.queue.open(rect, widget, kind);
        self.apply_queue();
        id
    }

    pub fn close_overlay(&mut self, id: OverlayId) {
        self.queue.close(id);
        self.apply_queue();
    }

    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    fn top_interactive(&self) -> Option<usize> {
        self.overlays.iter().rposition(|o| o.kind != OverlayKind::Tooltip)
    }

    /// Applies open/close requests; `true` if the set of overlays changed.
    fn apply_queue(&mut self) -> bool {
        let (closes, opens) = {
            let mut q = self.queue.0.borrow_mut();
            (std::mem::take(&mut q.close), std::mem::take(&mut q.open))
        };
        let changed = !closes.is_empty() || !opens.is_empty();
        for c in closes {
            let pos = match c {
                Some(id) => self.overlays.iter().position(|o| o.id == id),
                None => self.top_interactive(),
            };
            if let Some(i) = pos {
                self.overlays.remove(i);
            }
        }
        self.overlays.extend(opens);
        changed
    }

    /// Input for the topmost interactive overlay; `None` means the event goes to the tree.
    fn event_overlay(&mut self, ev: &UiEvent) -> Option<bool> {
        let idx = self.top_interactive()?;
        let env = self.env.clone();
        let sink = self.sink.clone();
        let o = &mut self.overlays[idx];
        let rect = o.rect;
        Some(match ev {
            UiEvent::PointerMove { pos } => {
                let inside = rect.contains(*pos);
                let mut ch = o.hover != inside;
                o.hover = inside;
                if inside {
                    ch |= o.widget.pointer_move(&env, rect, *pos);
                }
                if o.pressed {
                    o.widget.drag(&env, rect, *pos, &sink);
                    ch = true;
                }
                ch
            }
            UiEvent::PointerLeave => {
                o.hover = false;
                o.pressed = false;
                return None;
            }
            UiEvent::Wheel { pos, dx, dy } => rect.contains(*pos) && o.widget.wheel(&env, rect, *dx, *dy),
            UiEvent::PointerDown { pos, button: MouseButton::Left } => {
                if rect.contains(*pos) {
                    o.pressed = true;
                    o.widget.press(&env, rect, *pos, 1, &sink);
                } else {
                    self.overlays.remove(idx);
                }
                true
            }
            UiEvent::PointerUp { pos, button: MouseButton::Left } => {
                if std::mem::take(&mut o.pressed) && rect.contains(*pos) {
                    o.widget.click(&env, rect, *pos, &sink);
                }
                true
            }
            UiEvent::KeyDown { code, mods } => {
                if code == "Escape" {
                    self.overlays.remove(idx);
                } else if !o.widget.key(code, *mods, &sink) && (code == "Enter" || code == " ") {
                    o.widget.activate(&sink);
                }
                true
            }
            UiEvent::Text(t) => o.widget.text(t, &sink),
            _ => return None,
        })
    }

    pub fn root(&self) -> NodeId {
        self.tree.root()
    }

    /// Container without a widget (Row/Column/Stack).
    pub fn add_container(&mut self, parent: NodeId, style: Style) -> NodeId {
        self.tree.add(parent, style).expect("parent exists")
    }

    /// Leaf widget; `measure` returns the inner size for the available width.
    pub fn add_widget(
        &mut self,
        parent: NodeId,
        style: Style,
        widget: impl Widget<Msg> + 'static,
        measure: impl Fn(Option<f32>) -> Size + 'static,
    ) -> NodeId {
        let id = self.tree.add(parent, style).expect("parent exists");
        self.tree.set_measure(id, measure);
        self.nodes.insert(id, Node { widget: Box::new(widget), disabled: false });
        id
    }

    /// Container with a widget (no own measurement): size comes from the style and children.
    pub fn add_container_widget(&mut self, parent: NodeId, style: Style, widget: impl Widget<Msg> + 'static) -> NodeId {
        let id = self.tree.add(parent, style).expect("parent exists");
        self.nodes.insert(id, Node { widget: Box::new(widget), disabled: false });
        id
    }

    pub fn set_disabled(&mut self, id: NodeId, disabled: bool) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.disabled = disabled;
        }
        if disabled && self.focus == Some(id) {
            self.focus = None;
        }
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focus
    }
    /// Programmatically focuses a focusable, enabled widget (within the trap, if one is set).
    pub fn focus(&mut self, id: NodeId) -> bool {
        if self.focus_order().contains(&id) {
            self.focus = Some(id);
            true
        } else {
            false
        }
    }

    pub fn blur(&mut self) {
        self.focus = None;
    }

    /// Focus trap: Tab/Shift+Tab and clicks focus only widgets of the `scope` subtree.
    /// Focus outside the scope moves to its first element. `None` removes the trap.
    pub fn trap_focus(&mut self, scope: Option<NodeId>) {
        self.trap = scope;
        let order = self.focus_order();
        if self.focus.is_some_and(|f| !order.contains(&f)) {
            self.focus = order.first().copied();
        }
    }

    pub fn hovered(&self) -> Option<NodeId> {
        self.hover
    }

    pub fn state_of(&self, id: NodeId) -> WidgetState {
        let mut s = WidgetState::NORMAL;
        s.set(WidgetState::HOVERED, self.hover == Some(id));
        s.set(WidgetState::PRESSED, self.pressed == Some(id) && self.hover == Some(id));
        s.set(WidgetState::FOCUSED, self.focus == Some(id));
        s.set(WidgetState::DISABLED, self.nodes.get(&id).is_some_and(|n| n.disabled));
        s
    }

    fn enabled(&self, id: NodeId) -> bool {
        self.nodes.get(&id).is_some_and(|n| !n.disabled)
    }

    /// Focusable widgets in tree traversal order.
    fn focus_order(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![self.trap.unwrap_or_else(|| self.tree.root())];
        while let Some(id) = stack.pop() {
            if self.nodes.get(&id).is_some_and(|n| !n.disabled && n.widget.focusable()) {
                out.push(id);
            }
            stack.extend(self.tree.children(id).iter().rev().copied());
        }
        out
    }

    fn move_focus(&mut self, back: bool) {
        let order = self.focus_order();
        if order.is_empty() {
            self.focus = None;
            return;
        }
        let cur = self.focus.and_then(|f| order.iter().position(|&x| x == f));
        let n = order.len();
        let next = match (cur, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        self.focus = Some(order[next]);
    }

    fn activate(&mut self, id: NodeId) {
        if let Some(n) = self.nodes.get_mut(&id) {
            if !n.disabled {
                n.widget.activate(&self.sink);
            }
        }
    }

    fn click(&mut self, id: NodeId, pos: Point) {
        let rect = self.tree.rect(id);
        let env = self.env.clone();
        if let Some(n) = self.nodes.get_mut(&id) {
            if !n.disabled {
                n.widget.click(&env, rect, pos, &self.sink);
            }
        }
    }

    /// Calls a widget method with context; `true` if the widget exists and is enabled.
    fn with_widget(&mut self, id: NodeId, f: impl FnOnce(&mut dyn Widget<Msg>, &Env, Rect, &MsgSink<Msg>)) -> bool {
        let rect = self.tree.rect(id);
        let env = self.env.clone();
        match self.nodes.get_mut(&id) {
            Some(n) if !n.disabled => {
                f(n.widget.as_mut(), &env, rect, &self.sink);
                true
            }
            _ => false,
        }
    }

    fn key_to(&mut self, id: NodeId, code: &str, mods: Modifiers) -> bool {
        match self.nodes.get_mut(&id) {
            Some(n) if !n.disabled => n.widget.key(code, mods, &self.sink),
            _ => false,
        }
    }

    /// Removes a node with its subtree and cleans up references to it (hover/focus/keys).
    pub(crate) fn remove_subtree(&mut self, id: NodeId) {
        let mut stack = vec![id];
        let mut all = Vec::new();
        while let Some(n) = stack.pop() {
            all.push(n);
            stack.extend_from_slice(self.tree.children(n));
        }
        for n in &all {
            self.nodes.remove(n);
            self.keys.remove(n);
            if self.hover == Some(*n) {
                self.hover = None;
            }
            if self.pressed == Some(*n) {
                self.pressed = None;
            }
            if self.focus == Some(*n) {
                self.focus = None;
            }
            if self.trap == Some(*n) {
                self.trap = None;
            }
            if self.last_click.is_some_and(|c| c.0 == *n) {
                self.last_click = None;
            }
        }
        self.tree.remove(id);
    }

    pub(crate) fn replace_widget(&mut self, id: NodeId, widget: impl Widget<Msg> + 'static) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.widget = Box::new(widget);
        }
    }

    /// Handles an event. Returns `true` if a redraw is needed.
    pub fn event(&mut self, ev: &UiEvent) -> bool {
        let r = match self.event_overlay(ev) {
            Some(r) => r,
            None => self.event_tree(ev),
        };
        self.apply_queue() | r
    }

    fn event_tree(&mut self, ev: &UiEvent) -> bool {
        match ev {
            UiEvent::FileHovered(_) => !std::mem::replace(&mut self.file_hover, true),
            UiEvent::FileHoverCancelled => std::mem::replace(&mut self.file_hover, false),
            UiEvent::FileDropped(p) => {
                self.file_hover = false;
                if let Some(cb) = &self.file_drop {
                    cb.emit(&self.sink, p.clone());
                }
                true
            }
            UiEvent::PointerMove { pos } => {
                let a = self.set_hover(self.tree.hit_test(*pos));
                let b = match self.pressed {
                    Some(p) => self.with_widget(p, |w, env, rect, sink| w.drag(env, rect, *pos, sink)),
                    None => false,
                };
                a || b
            }
            UiEvent::Wheel { pos, dx, dy } => match self.tree.hit_test(*pos) {
                Some(id) if self.enabled(id) => {
                    let rect = self.tree.rect(id);
                    let env = self.env.clone();
                    self.nodes.get_mut(&id).is_some_and(|n| n.widget.wheel(&env, rect, *dx, *dy))
                }
                _ => false,
            },
            UiEvent::PointerLeave => {
                let a = self.set_hover(None);
                let b = self.pressed.take().is_some();
                a || b
            }
            UiEvent::PointerDown { pos, button: MouseButton::Left } => {
                let hit = self.tree.hit_test(*pos);
                self.set_hover(hit);
                self.pressed = hit.filter(|&i| self.enabled(i));
                if let Some(i) = self.pressed {
                    if self.nodes[&i].widget.focusable() && (self.trap.is_none() || self.focus_order().contains(&i)) {
                        self.focus = Some(i);
                    }
                    let now = Instant::now();
                    let clicks = match self.last_click {
                        Some((id, t, n)) if id == i && now.duration_since(t) < MULTI_CLICK => n + 1,
                        _ => 1,
                    };
                    self.last_click = Some((i, now, clicks));
                    self.with_widget(i, |w, env, rect, sink| w.press(env, rect, *pos, clicks, sink));
                }
                true
            }
            UiEvent::PointerUp { pos, button: MouseButton::Left } => {
                let hit = self.tree.hit_test(*pos);
                self.set_hover(hit);
                let pressed = self.pressed.take();
                if let (Some(p), Some(h)) = (pressed, hit) {
                    if p == h {
                        self.click(p, *pos);
                    }
                }
                true
            }
            UiEvent::KeyDown { code, mods } => match code.as_str() {
                "Tab" => {
                    self.move_focus(mods.shift);
                    true
                }
                c if self.focus.is_some_and(|f| self.key_to(f, c, *mods)) => true,
                "Enter" | " " => match self.focus {
                    Some(f) => {
                        self.activate(f);
                        true
                    }
                    None => false,
                },
                _ => false,
            },
            UiEvent::Text(t) => match self.focus {
                Some(f) => match self.nodes.get_mut(&f) {
                    Some(n) if !n.disabled => n.widget.text(t, &self.sink),
                    _ => false,
                },
                None => false,
            },
            _ => false,
        }
    }

    /// Cursor for the current pointer position (by the widget under it).
    pub fn cursor(&self) -> hey_core::CursorIcon {
        match self.hover.and_then(|h| self.nodes.get(&h).map(|n| (n.disabled, n.widget.cursor()))) {
            Some((true, _)) => hey_core::CursorIcon::NotAllowed,
            Some((false, c)) => c,
            None => hey_core::CursorIcon::Default,
        }
    }

    /// Message when a file is dropped onto the window (drag-and-drop).
    pub fn on_file_drop(&mut self, cb: Callback<Msg, std::path::PathBuf>) {
        self.file_drop = Some(cb);
    }

    /// A file is currently hovering over the window (for highlighting the drop zone).
    pub fn file_hovering(&self) -> bool {
        self.file_hover
    }

    fn set_hover(&mut self, h: Option<NodeId>) -> bool {
        if self.hover == h {
            return false;
        }
        self.hover = h;
        true
    }

    /// Layout and paint of all widgets in tree order.
    pub fn paint(&mut self, viewport: Size) -> DisplayList {
        self.tree.compute(viewport);
        let mut out = DisplayList::new();
        let mut stack = vec![self.tree.root()];
        while let Some(id) = stack.pop() {
            let rect = self.tree.rect(id);
            if let Some(n) = self.nodes.get(&id) {
                let st = self.state_of(id);
                n.widget.paint(&self.env, rect, st, &mut out);
                if st.focused() {
                    self.paint_focus(rect, &mut out);
                }
            }
            stack.extend(self.tree.children(id).iter().rev().copied());
        }
        self.queue.0.borrow_mut().viewport = viewport;
        self.apply_queue();
        let top = self.top_interactive();
        for (i, o) in self.overlays.iter().enumerate() {
            if o.kind == OverlayKind::Modal {
                out.push(DrawCmd::fill(Rect::new(0.0, 0.0, viewport.w, viewport.h), hey_core::Color::rgba(0, 0, 0, 150)));
            }
            let mut st = WidgetState::NORMAL;
            st.set(WidgetState::HOVERED, o.hover);
            st.set(WidgetState::PRESSED, o.pressed && o.hover);
            st.set(WidgetState::FOCUSED, top == Some(i));
            o.widget.paint(&self.env, o.rect, st, &mut out);
        }
        out
    }

    fn paint_focus(&self, rect: Rect, out: &mut DisplayList) {
        let t = &self.env.theme;
        let o = t.focus_offset + t.focus_width / 2.0;
        let r = Rect::new(rect.x - o, rect.y - o, rect.w + 2.0 * o, rect.h + 2.0 * o);
        out.push(DrawCmd::Rect {
            rect: r,
            fill: hey_core::Color::TRANSPARENT,
            border: Some(Border { width: t.focus_width, color: t.palette.focus }),
            chamfer: t.chamfer_small(),
            shadow: None,
        });
    }

    pub fn hit(&self, p: Point) -> Option<NodeId> {
        self.tree.hit_test(p)
    }
}
