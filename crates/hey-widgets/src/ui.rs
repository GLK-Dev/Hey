// Copyright (c) Vitaly Golik (mjojo)

use hey_core::{MouseButton, MsgSink, NodeId, Point, Rect, Size, UiEvent};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::{LayoutTree, Style};
use hey_style::{Theme, WidgetState};
use hey_text::{FontId, Fonts};
use std::collections::HashMap;
use std::rc::Rc;

/// Контекст отрисовки/измерения, общий для всех виджетов.
pub struct Env {
    pub theme: Theme,
    pub fonts: Rc<Fonts>,
    pub chain: Vec<FontId>,
    /// Базовый размер шрифта (физ. px).
    pub font_px: f32,
}

/// Поведение виджета. Состояние (hover/pressed/focus/disabled) хранит `Ui`.
pub trait Widget<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList);
    fn focusable(&self) -> bool {
        false
    }
    /// Клик мышью или Enter/Space на сфокусированном виджете.
    fn activate(&mut self, _sink: &MsgSink<Msg>) {}
    /// Клик по виджету (позиция в абсолютных координатах). По умолчанию — `activate`.
    fn click(&mut self, _env: &Env, _rect: Rect, _pos: Point, sink: &MsgSink<Msg>) {
        self.activate(sink);
    }
    /// Клавиша на сфокусированном виджете; `true` — обработана.
    fn key(&mut self, _code: &str, _sink: &MsgSink<Msg>) -> bool {
        false
    }
}

struct Node<Msg> {
    widget: Box<dyn Widget<Msg>>,
    disabled: bool,
}

pub struct Ui<Msg> {
    pub tree: LayoutTree,
    pub env: Rc<Env>,
    sink: MsgSink<Msg>,
    nodes: HashMap<NodeId, Node<Msg>>,
    hover: Option<NodeId>,
    pressed: Option<NodeId>,
    focus: Option<NodeId>,
}

impl<Msg: 'static> Ui<Msg> {
    pub fn new(theme: Theme, fonts: Rc<Fonts>, chain: Vec<FontId>, sink: MsgSink<Msg>) -> Self {
        let font_px = theme.space(14.0);
        Ui {
            tree: LayoutTree::new(Style::column()),
            env: Rc::new(Env { theme, fonts, chain, font_px }),
            sink,
            nodes: HashMap::new(),
            hover: None,
            pressed: None,
            focus: None,
        }
    }

    pub fn root(&self) -> NodeId {
        self.tree.root()
    }

    /// Контейнер без виджета (Row/Column/Stack).
    pub fn add_container(&mut self, parent: NodeId, style: Style) -> NodeId {
        self.tree.add(parent, style).expect("родитель существует")
    }

    /// Листовой виджет; `measure` — внутренний размер по доступной ширине.
    pub fn add_widget(
        &mut self,
        parent: NodeId,
        style: Style,
        widget: impl Widget<Msg> + 'static,
        measure: impl Fn(Option<f32>) -> Size + 'static,
    ) -> NodeId {
        let id = self.tree.add(parent, style).expect("родитель существует");
        self.tree.set_measure(id, measure);
        self.nodes.insert(id, Node { widget: Box::new(widget), disabled: false });
        id
    }

    /// Контейнер с виджетом (без собственного измерения): размер задают стиль и дети.
    pub fn add_container_widget(&mut self, parent: NodeId, style: Style, widget: impl Widget<Msg> + 'static) -> NodeId {
        let id = self.tree.add(parent, style).expect("родитель существует");
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

    /// Фокусируемые виджеты в порядке обхода дерева.
    fn focus_order(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![self.tree.root()];
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

    fn key_to(&mut self, id: NodeId, code: &str) -> bool {
        match self.nodes.get_mut(&id) {
            Some(n) if !n.disabled => n.widget.key(code, &self.sink),
            _ => false,
        }
    }

    pub(crate) fn replace_widget(&mut self, id: NodeId, widget: impl Widget<Msg> + 'static) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.widget = Box::new(widget);
        }
    }

    /// Обрабатывает событие. Возвращает `true`, если нужна перерисовка.
    pub fn event(&mut self, ev: &UiEvent) -> bool {
        match ev {
            UiEvent::PointerMove { pos } => self.set_hover(self.tree.hit_test(*pos)),
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
                    if self.nodes[&i].widget.focusable() {
                        self.focus = Some(i);
                    }
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
                c if self.focus.is_some_and(|f| self.key_to(f, c)) => true,
                "Enter" | " " => match self.focus {
                    Some(f) => {
                        self.activate(f);
                        true
                    }
                    None => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    fn set_hover(&mut self, h: Option<NodeId>) -> bool {
        if self.hover == h {
            return false;
        }
        self.hover = h;
        true
    }

    /// Раскладка и отрисовка всех виджетов в порядке дерева.
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
