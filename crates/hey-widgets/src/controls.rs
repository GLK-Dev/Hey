// Copyright (c) Vitaly Golik (mjojo)

use crate::a11y::{A11y, Role};
use crate::ui::{Env, Ui, Widget};
use crate::widgets::text_style;
use hey_core::{Callback, Modifiers, MsgSink, NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::Style;
use hey_style::WidgetState;
use hey_text::layout;
use std::cell::Cell;
use std::rc::Rc;

fn box_size(env: &Env) -> f32 {
    (env.font_px * 1.2).round()
}

/// Label to the right of an icon; returns the size of the whole "icon + label" row.
fn measure_row(env: &Env, label: &str, icon_w: f32) -> Size {
    let l = layout(&env.fonts, label, &text_style(env, None, false));
    let gap = if label.is_empty() { 0.0 } else { env.theme.space(8.0) };
    Size::new(icon_w + gap + l.width.ceil(), l.height.ceil().max(box_size(env)))
}

fn paint_label(env: &Env, rect: Rect, label: &str, icon_w: f32, state: WidgetState, out: &mut DisplayList) {
    if label.is_empty() {
        return;
    }
    let color = if state.disabled() { env.theme.palette.text_disabled } else { env.theme.palette.text };
    let l = layout(&env.fonts, label, &text_style(env, None, false));
    let origin = Point::new(rect.x + icon_w + env.theme.space(8.0), rect.y + (rect.h - l.height) / 2.0);
    for run in l.to_runs(origin, color) {
        out.push(DrawCmd::Text(run));
    }
}

fn icon_rect(env: &Env, rect: Rect, w: f32) -> Rect {
    Rect::new(rect.x, rect.y + (rect.h - box_size(env)) / 2.0, w, box_size(env))
}

fn frame(env: &Env, r: Rect, on: bool, state: WidgetState, out: &mut DisplayList) {
    let p = &env.theme.palette;
    let line = if state.focused() || state.hovered() { p.focus } else { p.line };
    out.push(DrawCmd::Rect {
        rect: r,
        fill: if state.disabled() {
            p.panel
        } else if on {
            p.focus
        } else {
            p.document
        },
        border: Some(Border { width: 1.0, color: line }),
        chamfer: env.theme.chamfer_small(),
        shadow: None,
    });
}

/// Checkbox: activation (click, Enter, Space) toggles it and sends the new value.
pub struct Checkbox<Msg> {
    label: String,
    checked: bool,
    on_change: Callback<Msg, bool>,
}

impl<Msg: 'static> Checkbox<Msg> {
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, label: &str, checked: bool, on_change: Callback<Msg, bool>) -> NodeId {
        let env = ui.env.clone();
        let t = label.to_string();
        let m = move |_| measure_row(&env, &t, box_size(&env));
        ui.add_widget(parent, Style::leaf(), Checkbox { label: label.into(), checked, on_change }, m)
    }
}

impl<Msg: 'static> Widget<Msg> for Checkbox<Msg> {
    fn a11y(&self) -> Option<A11y> {
        Some(A11y::new(Role::CheckBox, self.label.clone()).toggled(self.checked))
    }
    fn cursor(&self) -> hey_core::CursorIcon {
        hey_core::CursorIcon::Pointer
    }
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let s = box_size(env);
        let r = icon_rect(env, rect, s);
        frame(env, r, self.checked, state, out);
        if self.checked {
            let c = env.theme.palette.document;
            let (a, b, d) = (
                Point::new(r.x + s * 0.25, r.y + s * 0.52),
                Point::new(r.x + s * 0.43, r.y + s * 0.70),
                Point::new(r.x + s * 0.75, r.y + s * 0.30),
            );
            out.push(DrawCmd::Line { a, b, width: 2.0, color: c });
            out.push(DrawCmd::Line { a: b, b: d, width: 2.0, color: c });
        }
        paint_label(env, rect, &self.label, s, state, out);
    }
    fn focusable(&self) -> bool {
        true
    }
    fn activate(&mut self, sink: &MsgSink<Msg>) {
        self.checked = !self.checked;
        self.on_change.emit(sink, self.checked);
    }
}

/// Switch (toggle): like a checkbox, but with a "thumb".
pub struct Toggle<Msg> {
    label: String,
    on: bool,
    on_change: Callback<Msg, bool>,
}

impl<Msg: 'static> Toggle<Msg> {
    fn track_w(env: &Env) -> f32 {
        box_size(env) * 2.0
    }

    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, label: &str, on: bool, on_change: Callback<Msg, bool>) -> NodeId {
        let env = ui.env.clone();
        let t = label.to_string();
        let m = move |_| measure_row(&env, &t, Self::track_w(&env));
        ui.add_widget(parent, Style::leaf(), Toggle { label: label.into(), on, on_change }, m)
    }
}

impl<Msg: 'static> Widget<Msg> for Toggle<Msg> {
    fn a11y(&self) -> Option<A11y> {
        Some(A11y::new(Role::Switch, self.label.clone()).toggled(self.on))
    }
    fn cursor(&self) -> hey_core::CursorIcon {
        hey_core::CursorIcon::Pointer
    }
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let w = box_size(env) * 2.0;
        let r = icon_rect(env, rect, w);
        frame(env, r, self.on, state, out);
        let pad = 3.0;
        let k = r.h - 2.0 * pad;
        let x = if self.on { r.x + r.w - pad - k } else { r.x + pad };
        let p = &env.theme.palette;
        let knob = if self.on { p.document } else { p.text };
        out.push(DrawCmd::Rect {
            rect: Rect::new(x, r.y + pad, k, k),
            fill: if state.disabled() { p.text_disabled } else { knob },
            border: None,
            chamfer: env.theme.chamfer_small(),
            shadow: None,
        });
        paint_label(env, rect, &self.label, w, state, out);
    }
    fn focusable(&self) -> bool {
        true
    }
    fn activate(&mut self, sink: &MsgSink<Msg>) {
        self.on = !self.on;
        self.on_change.emit(sink, self.on);
    }
}

/// Shared value of a radio group: index of the selected one (or `None`).
pub type RadioGroup = Rc<Cell<Option<usize>>>;

/// Radio button: the choice is written to the shared `RadioGroup`, the index is sent.
pub struct Radio<Msg> {
    label: String,
    index: usize,
    group: RadioGroup,
    on_select: Callback<Msg, usize>,
}

impl<Msg: 'static> Radio<Msg> {
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, label: &str, index: usize, group: RadioGroup, on_select: Callback<Msg, usize>) -> NodeId {
        let env = ui.env.clone();
        let t = label.to_string();
        let m = move |_| measure_row(&env, &t, box_size(&env));
        ui.add_widget(parent, Style::leaf(), Radio { label: label.into(), index, group, on_select }, m)
    }
}

impl<Msg: 'static> Widget<Msg> for Radio<Msg> {
    fn a11y(&self) -> Option<A11y> {
        Some(A11y::new(Role::RadioButton, self.label.clone()).toggled(self.group.get() == Some(self.index)))
    }
    fn cursor(&self) -> hey_core::CursorIcon {
        hey_core::CursorIcon::Pointer
    }
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let s = box_size(env);
        let r = icon_rect(env, rect, s);
        let on = self.group.get() == Some(self.index);
        frame(env, r, false, state, out);
        if on {
            let d = s * 0.5;
            out.push(DrawCmd::Rect {
                rect: Rect::new(r.x + (s - d) / 2.0, r.y + (s - d) / 2.0, d, d),
                fill: env.theme.palette.focus,
                border: None,
                chamfer: env.theme.chamfer_small(),
                shadow: None,
            });
        }
        paint_label(env, rect, &self.label, s, state, out);
    }
    fn focusable(&self) -> bool {
        true
    }
    fn activate(&mut self, sink: &MsgSink<Msg>) {
        if self.group.get() != Some(self.index) {
            self.group.set(Some(self.index));
            self.on_select.emit(sink, self.index);
        }
    }
}

/// Slider over `min..=max`: mouse (press and drag) and keys
/// ←/→ (step of 5% of the range), Home/End.
pub struct Slider<Msg> {
    min: f32,
    max: f32,
    value: f32,
    on_change: Callback<Msg, f32>,
}

impl<Msg: 'static> Slider<Msg> {
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, min: f32, max: f32, value: f32, width: f32, on_change: Callback<Msg, f32>) -> NodeId {
        let env = ui.env.clone();
        let h = (env.font_px * 1.6).round();
        let width = env.theme.space(width);
        let w = Slider { min, max, value: value.clamp(min, max), on_change };
        ui.add_widget(parent, Style::leaf(), w, move |_| Size::new(width, h))
    }

    fn thumb(env: &Env) -> f32 {
        box_size(env)
    }

    fn set(&mut self, v: f32, sink: &MsgSink<Msg>) {
        let v = v.clamp(self.min, self.max);
        if v != self.value {
            self.value = v;
            self.on_change.emit(sink, v);
        }
    }

    fn value_at_x(&self, env: &Env, rect: Rect, x: f32) -> f32 {
        let th = Self::thumb(env);
        let span = (rect.w - th).max(1.0);
        let t = ((x - rect.x - th / 2.0) / span).clamp(0.0, 1.0);
        self.min + t * (self.max - self.min)
    }
}

impl<Msg: 'static> Widget<Msg> for Slider<Msg> {
    fn a11y(&self) -> Option<A11y> {
        Some(A11y::new(Role::Slider, "").range(self.min as f64, self.max as f64, self.value as f64))
    }
    fn cursor(&self) -> hey_core::CursorIcon {
        hey_core::CursorIcon::Pointer
    }
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let p = &env.theme.palette;
        let th = Self::thumb(env);
        let t = if self.max > self.min { (self.value - self.min) / (self.max - self.min) } else { 0.0 };
        let tx = rect.x + t * (rect.w - th);
        let (ty, tk) = (rect.y + rect.h / 2.0 - 2.0, 4.0);
        out.push(DrawCmd::fill(Rect::new(rect.x, ty, rect.w, tk), p.line));
        out.push(DrawCmd::fill(
            Rect::new(rect.x, ty, tx - rect.x + th / 2.0, tk),
            if state.disabled() { p.text_disabled } else { p.focus },
        ));
        out.push(DrawCmd::Rect {
            rect: Rect::new(tx, rect.y + (rect.h - th) / 2.0, th, th),
            fill: if state.pressed() { p.focus } else { p.text },
            border: Some(Border { width: 1.0, color: if state.focused() { p.focus } else { p.line } }),
            chamfer: env.theme.chamfer_small(),
            shadow: None,
        });
    }
    fn focusable(&self) -> bool {
        true
    }
    fn click(&mut self, _env: &Env, _rect: Rect, _pos: Point, _sink: &MsgSink<Msg>) {}
    fn press(&mut self, env: &Env, rect: Rect, pos: Point, _clicks: u32, sink: &MsgSink<Msg>) {
        let v = self.value_at_x(env, rect, pos.x);
        self.set(v, sink);
    }
    fn drag(&mut self, env: &Env, rect: Rect, pos: Point, sink: &MsgSink<Msg>) {
        let v = self.value_at_x(env, rect, pos.x);
        self.set(v, sink);
    }
    fn key(&mut self, code: &str, _mods: Modifiers, sink: &MsgSink<Msg>) -> bool {
        let step = (self.max - self.min) * 0.05;
        let v = match code {
            "ArrowLeft" | "ArrowDown" => self.value - step,
            "ArrowRight" | "ArrowUp" => self.value + step,
            "Home" => self.min,
            "End" => self.max,
            _ => return false,
        };
        self.set(v, sink);
        true
    }
}

/// Progress indicator `0..=1` (display only). The value is changed via [`ProgressBar::set`].
pub struct ProgressBar {
    value: f32,
}

impl ProgressBar {
    pub fn add<Msg: 'static>(ui: &mut Ui<Msg>, parent: NodeId, value: f32, width: f32) -> NodeId {
        let h = (ui.env.font_px * 0.5).round().max(4.0);
        let width = ui.env.theme.space(width);
        ui.add_widget(parent, Style::leaf(), ProgressBar { value: value.clamp(0.0, 1.0) }, move |_| Size::new(width, h))
    }

    pub fn set<Msg: 'static>(ui: &mut Ui<Msg>, id: NodeId, value: f32) {
        ui.replace_widget(id, ProgressBar { value: value.clamp(0.0, 1.0) });
    }
}

impl<Msg: 'static> Widget<Msg> for ProgressBar {
    fn a11y(&self) -> Option<A11y> {
        Some(A11y::new(Role::ProgressBar, "").range(0.0, 1.0, self.value as f64))
    }
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let p = &env.theme.palette;
        out.push(DrawCmd::fill(rect, p.line));
        if self.value > 0.0 {
            let c = if state.disabled() { p.text_disabled } else { p.focus };
            out.push(DrawCmd::fill(Rect::new(rect.x, rect.y, rect.w * self.value, rect.h), c));
        }
    }
}
