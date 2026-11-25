// Copyright (c) Vitaly Golik (mjojo)

use crate::ui::{Env, OverlayId, OverlayKind, Overlays, Ui, Widget};
use crate::widgets::text_style;
use hey_core::{Callback, Modifiers, MsgSink, NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::Style;
use hey_style::WidgetState;
use hey_text::layout;
use std::cell::Cell;
use std::rc::Rc;

fn row_height(env: &Env) -> f32 {
    (layout(&env.fonts, "Ag", &text_style(env, None, false)).height + 2.0 * env.theme.space(4.0)).ceil()
}

fn text_width(env: &Env, s: &str) -> f32 {
    layout(&env.fonts, s, &text_style(env, None, false)).width.ceil()
}

fn push_text(env: &Env, s: &str, origin: Point, color: hey_core::Color, out: &mut DisplayList) {
    for run in layout(&env.fonts, s, &text_style(env, None, false)).to_runs(origin, color) {
        out.push(DrawCmd::Text(run));
    }
}

fn panel(env: &Env, rect: Rect, out: &mut DisplayList) {
    let p = &env.theme.palette;
    out.push(DrawCmd::Rect {
        rect,
        fill: p.panel,
        border: Some(Border { width: 1.0, color: p.line }),
        chamfer: env.theme.chamfer_small(),
        shadow: None,
    });
}

/// Всплывающее меню выбора строки. Закрывается выбором, Esc или кликом снаружи.
pub struct Menu<Msg> {
    items: Vec<String>,
    hot: usize,
    on_select: Callback<Msg, usize>,
    overlays: Overlays<Msg>,
    /// Общее с `Dropdown` текущее значение (обновляется при выборе).
    shared: Option<Rc<Cell<usize>>>,
}

impl<Msg: 'static> Menu<Msg> {
    fn rect_for(env: &Env, items: &[String], at: Point, min_w: f32) -> Rect {
        let pad = env.theme.space(10.0);
        let w = items.iter().map(|s| text_width(env, s)).fold(0.0, f32::max) + 2.0 * pad;
        Rect::new(at.x, at.y, w.max(min_w), row_height(env) * items.len() as f32 + 2.0)
    }

    /// Открывает меню в точке `at` (верхний левый угол).
    pub fn open(ui: &mut Ui<Msg>, at: Point, items: Vec<String>, on_select: Callback<Msg, usize>) -> OverlayId {
        let rect = Self::rect_for(&ui.env, &items, at, 0.0);
        let m = Menu { items, hot: 0, on_select, overlays: ui.overlays(), shared: None };
        ui.open_overlay(rect, m, OverlayKind::Popup)
    }

    fn choose(&mut self, i: usize, sink: &MsgSink<Msg>) {
        if let Some(s) = &self.shared {
            s.set(i);
        }
        self.on_select.emit(sink, i);
        self.overlays.close_top();
    }

    fn row_at(&self, env: &Env, rect: Rect, pos: Point) -> Option<usize> {
        let i = ((pos.y - rect.y - 1.0) / row_height(env)).floor();
        (i >= 0.0 && (i as usize) < self.items.len()).then_some(i as usize)
    }
}

impl<Msg: 'static> Widget<Msg> for Menu<Msg> {
    fn paint(&self, env: &Env, rect: Rect, _state: WidgetState, out: &mut DisplayList) {
        panel(env, rect, out);
        let p = &env.theme.palette;
        let rh = row_height(env);
        for (i, s) in self.items.iter().enumerate() {
            let y = rect.y + 1.0 + i as f32 * rh;
            if i == self.hot {
                out.push(DrawCmd::fill(Rect::new(rect.x + 1.0, y, rect.w - 2.0, rh), p.selection));
            }
            let l = layout(&env.fonts, s, &text_style(env, None, false));
            push_text(env, s, Point::new(rect.x + env.theme.space(10.0), y + (rh - l.height) / 2.0), p.text, out);
        }
    }
    fn focusable(&self) -> bool {
        true
    }
    fn pointer_move(&mut self, env: &Env, rect: Rect, pos: Point) -> bool {
        match self.row_at(env, rect, pos) {
            Some(i) if i != self.hot => {
                self.hot = i;
                true
            }
            _ => false,
        }
    }
    fn press(&mut self, env: &Env, rect: Rect, pos: Point, _clicks: u32, _sink: &MsgSink<Msg>) {
        if let Some(i) = self.row_at(env, rect, pos) {
            self.hot = i;
        }
    }
    fn click(&mut self, env: &Env, rect: Rect, pos: Point, sink: &MsgSink<Msg>) {
        if let Some(i) = self.row_at(env, rect, pos) {
            self.choose(i, sink);
        }
    }
    fn activate(&mut self, sink: &MsgSink<Msg>) {
        if !self.items.is_empty() {
            self.choose(self.hot, sink);
        }
    }
    fn key(&mut self, code: &str, _mods: Modifiers, _sink: &MsgSink<Msg>) -> bool {
        let last = self.items.len().saturating_sub(1);
        self.hot = match code {
            "ArrowDown" => (self.hot + 1).min(last),
            "ArrowUp" => self.hot.saturating_sub(1),
            "Home" => 0,
            "End" => last,
            _ => return false,
        };
        true
    }
}

/// Выпадающий список: кнопка с текущим значением, по активации раскрывает `Menu` под собой.
pub struct Dropdown<Msg> {
    items: Vec<String>,
    selected: Rc<Cell<usize>>,
    rect: Cell<Rect>,
    on_select: Callback<Msg, usize>,
    overlays: Overlays<Msg>,
}

impl<Msg: 'static> Dropdown<Msg> {
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, items: Vec<String>, selected: usize, width: f32, on_select: Callback<Msg, usize>) -> NodeId {
        let env = ui.env.clone();
        let h = row_height(&env) + 2.0 * env.theme.space(2.0);
        let d = Dropdown {
            selected: Rc::new(Cell::new(selected.min(items.len().saturating_sub(1)))),
            items,
            rect: Cell::new(Rect::new(0.0, 0.0, width, h)),
            on_select,
            overlays: ui.overlays(),
        };
        ui.add_widget(parent, Style::leaf(), d, move |_| Size::new(width, h))
    }

    fn open(&self, env: &Env) {
        let r = self.rect.get();
        let mut menu_rect = Menu::<Msg>::rect_for(env, &self.items, Point::new(r.x, r.bottom() + 2.0), r.w);
        menu_rect.w = menu_rect.w.max(r.w);
        let menu = Menu {
            items: self.items.clone(),
            hot: self.selected.get(),
            on_select: self.on_select.clone(),
            overlays: self.overlays.clone(),
            shared: Some(self.selected.clone()),
        };
        self.overlays.open(menu_rect, menu, OverlayKind::Popup);
    }
}

impl<Msg: 'static> Widget<Msg> for Dropdown<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        self.rect.set(rect);
        let p = &env.theme.palette;
        out.push(DrawCmd::Rect {
            rect,
            fill: if state.disabled() { p.panel } else { p.document },
            border: Some(Border { width: 1.0, color: if state.focused() || state.hovered() { p.focus } else { p.line } }),
            chamfer: env.theme.chamfer_small(),
            shadow: None,
        });
        let color = if state.disabled() { p.text_disabled } else { p.text };
        let s = self.items.get(self.selected.get()).map(String::as_str).unwrap_or("");
        let l = layout(&env.fonts, s, &text_style(env, None, false));
        push_text(env, s, Point::new(rect.x + env.theme.space(10.0), rect.y + (rect.h - l.height) / 2.0), color, out);
        // шеврон ▾
        let (cx, cy, k) = (rect.right() - env.theme.space(14.0), rect.y + rect.h / 2.0, 4.0);
        out.push(DrawCmd::Line { a: Point::new(cx - k, cy - k / 2.0), b: Point::new(cx, cy + k / 2.0), width: 1.5, color });
        out.push(DrawCmd::Line { a: Point::new(cx, cy + k / 2.0), b: Point::new(cx + k, cy - k / 2.0), width: 1.5, color });
    }
    fn focusable(&self) -> bool {
        true
    }
    fn click(&mut self, env: &Env, _rect: Rect, _pos: Point, _sink: &MsgSink<Msg>) {
        self.open(env);
    }
    fn activate(&mut self, _sink: &MsgSink<Msg>) {
        // клавиатурная активация: окружение берём из размеров последней отрисовки
        self.open_kbd();
    }
}

impl<Msg: 'static> Dropdown<Msg> {
    fn open_kbd(&self) {
        let r = self.rect.get();
        let menu = Menu {
            items: self.items.clone(),
            hot: self.selected.get(),
            on_select: self.on_select.clone(),
            overlays: self.overlays.clone(),
            shared: Some(self.selected.clone()),
        };
        // высота строки неизвестна без Env: приблизительно по высоте кнопки
        let rows = self.items.len() as f32;
        let w = r.w;
        self.overlays.open(Rect::new(r.x, r.bottom() + 2.0, w, (r.h - 4.0).max(1.0) * rows + 2.0), menu, OverlayKind::Popup);
    }
}

/// Подсказка у якоря. Не принимает ввод; закрывается через `Ui::close_overlay`.
pub struct Tooltip {
    text: String,
}

impl Tooltip {
    /// Показывает подсказку под `anchor`.
    pub fn show<Msg: 'static>(ui: &mut Ui<Msg>, anchor: Rect, text: &str) -> OverlayId {
        let env = ui.env.clone();
        let pad = env.theme.space(8.0);
        let l = layout(&env.fonts, text, &text_style(&env, None, false));
        let rect = Rect::new(anchor.x, anchor.bottom() + 4.0, l.width.ceil() + 2.0 * pad, l.height.ceil() + pad);
        ui.open_overlay(rect, Tooltip { text: text.into() }, OverlayKind::Tooltip)
    }
}

impl<Msg: 'static> Widget<Msg> for Tooltip {
    fn paint(&self, env: &Env, rect: Rect, _state: WidgetState, out: &mut DisplayList) {
        panel(env, rect, out);
        let pad = env.theme.space(4.0);
        push_text(env, &self.text, Point::new(rect.x + 2.0 * pad, rect.y + pad), env.theme.palette.text, out);
    }
}

/// Модальное окно с сообщением и кнопками. Выбор — `Callback<Msg, usize>` (индекс кнопки);
/// Esc закрывает без выбора. Tab заперт внутри (оверлей — единственная цель ввода).
pub struct Modal<Msg> {
    title: String,
    message: String,
    buttons: Vec<String>,
    hot: usize,
    on_choose: Callback<Msg, usize>,
    overlays: Overlays<Msg>,
}

impl<Msg: 'static> Modal<Msg> {
    pub fn open(ui: &mut Ui<Msg>, title: &str, message: &str, buttons: Vec<String>, on_choose: Callback<Msg, usize>) -> OverlayId {
        let env = ui.env.clone();
        let t = &env.theme;
        let vp = ui.overlays().viewport();
        let w = t.space(360.0);
        let ml = layout(&env.fonts, message, &text_style(&env, Some(w - 2.0 * t.space(16.0)), true));
        let h = t.space(16.0) * 2.0 + row_height(&env) + t.space(8.0) + ml.height.ceil() + t.space(16.0) + row_height(&env) + t.space(4.0);
        let rect = Rect::new(((vp.w - w) / 2.0).max(0.0), ((vp.h - h) / 2.0).max(0.0), w, h);
        let m = Modal { title: title.into(), message: message.into(), hot: 0, buttons, on_choose, overlays: ui.overlays() };
        ui.open_overlay(rect, m, OverlayKind::Modal)
    }

    fn button_rects(&self, env: &Env, rect: Rect) -> Vec<Rect> {
        let t = &env.theme;
        let h = row_height(env) + t.space(4.0);
        let mut x = rect.right() - t.space(16.0);
        let mut v: Vec<Rect> = self
            .buttons
            .iter()
            .rev()
            .map(|b| {
                let w = text_width(env, b) + 2.0 * t.space(14.0);
                x -= w;
                let r = Rect::new(x, rect.bottom() - t.space(16.0) - h, w, h);
                x -= t.space(8.0);
                r
            })
            .collect();
        v.reverse();
        v
    }

    fn choose(&mut self, i: usize, sink: &MsgSink<Msg>) {
        self.on_choose.emit(sink, i);
        self.overlays.close_top();
    }
}

impl<Msg: 'static> Widget<Msg> for Modal<Msg> {
    fn paint(&self, env: &Env, rect: Rect, _state: WidgetState, out: &mut DisplayList) {
        panel(env, rect, out);
        let t = &env.theme;
        let p = &t.palette;
        let pad = t.space(16.0);
        push_text(env, &self.title, Point::new(rect.x + pad, rect.y + pad), p.text, out);
        let my = rect.y + pad + row_height(env) + t.space(8.0);
        let ml = layout(&env.fonts, &self.message, &text_style(env, Some(rect.w - 2.0 * pad), true));
        for run in ml.to_runs(Point::new(rect.x + pad, my), p.text_disabled) {
            out.push(DrawCmd::Text(run));
        }
        for (i, r) in self.button_rects(env, rect).into_iter().enumerate() {
            out.push(DrawCmd::Rect {
                rect: r,
                fill: if i == self.hot { p.selection } else { p.document },
                border: Some(Border { width: 1.0, color: if i == self.hot { p.focus } else { p.line } }),
                chamfer: t.chamfer_small(),
                shadow: None,
            });
            let l = layout(&env.fonts, &self.buttons[i], &text_style(env, None, false));
            push_text(env, &self.buttons[i], Point::new(r.x + (r.w - l.width) / 2.0, r.y + (r.h - l.height) / 2.0), p.text, out);
        }
    }
    fn focusable(&self) -> bool {
        true
    }
    fn pointer_move(&mut self, env: &Env, rect: Rect, pos: Point) -> bool {
        match self.button_rects(env, rect).iter().position(|r| r.contains(pos)) {
            Some(i) if i != self.hot => {
                self.hot = i;
                true
            }
            _ => false,
        }
    }
    fn click(&mut self, env: &Env, rect: Rect, pos: Point, sink: &MsgSink<Msg>) {
        if let Some(i) = self.button_rects(env, rect).iter().position(|r| r.contains(pos)) {
            self.choose(i, sink);
        }
    }
    fn activate(&mut self, sink: &MsgSink<Msg>) {
        if !self.buttons.is_empty() {
            self.choose(self.hot, sink);
        }
    }
    fn key(&mut self, code: &str, _mods: Modifiers, _sink: &MsgSink<Msg>) -> bool {
        let last = self.buttons.len().saturating_sub(1);
        self.hot = match code {
            "ArrowLeft" => self.hot.saturating_sub(1),
            "ArrowRight" => (self.hot + 1).min(last),
            _ => return false,
        };
        true
    }
}
