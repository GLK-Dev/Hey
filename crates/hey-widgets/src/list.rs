// Copyright (c) Vitaly Golik (mjojo)

use crate::ui::{Env, Ui, Widget};
use crate::widgets::text_style;
use hey_core::{Callback, Modifiers, MsgSink, NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::Style;
use hey_style::WidgetState;
use hey_text::layout;
use std::cell::Cell;

/// Список строк с одиночным выбором: прокрутка колесом, стрелки/Home/End/PageUp/PageDown,
/// выбор кликом. Рисуются только видимые строки. Выбор — `Callback<Msg, usize>`.
pub struct List<Msg> {
    items: Vec<String>,
    selected: Option<usize>,
    on_select: Callback<Msg, usize>,
    scroll: Cell<f32>,
    follow: Cell<bool>,
    /// Высота строки и видимая высота при последней отрисовке.
    row_h: Cell<f32>,
    view_h: Cell<f32>,
}

impl<Msg: 'static> List<Msg> {
    /// `rows` — высота в видимых строках.
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, items: Vec<String>, width: f32, rows: usize, on_select: Callback<Msg, usize>) -> NodeId {
        let env = ui.env.clone();
        let row_h = Self::row_height(&env);
        let h = row_h * rows as f32 + 2.0;
        let w = List {
            items,
            selected: None,
            on_select,
            scroll: Cell::new(0.0),
            follow: Cell::new(false),
            row_h: Cell::new(row_h),
            view_h: Cell::new(h - 2.0),
        };
        ui.add_widget(parent, Style::leaf(), w, move |_| Size::new(width, h))
    }

    fn row_height(env: &Env) -> f32 {
        (layout(&env.fonts, "Ag", &text_style(env, None, false)).height + 2.0 * env.theme.space(4.0)).ceil()
    }

    fn max_scroll(&self) -> f32 {
        (self.items.len() as f32 * self.row_h.get() - self.view_h.get()).max(0.0)
    }

    fn select(&mut self, i: usize, sink: &MsgSink<Msg>) {
        if self.selected != Some(i) {
            self.selected = Some(i);
            self.on_select.emit(sink, i);
        }
        self.follow.set(true);
    }
}

impl<Msg: 'static> Widget<Msg> for List<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let t = &env.theme;
        let p = &t.palette;
        out.push(DrawCmd::Rect {
            rect,
            fill: if state.disabled() { p.panel } else { p.document },
            border: Some(Border { width: 1.0, color: if state.focused() { p.focus } else { p.line } }),
            chamfer: t.chamfer_small(),
            shadow: None,
        });
        let rh = Self::row_height(env);
        let view_h = (rect.h - 2.0).max(1.0);
        self.row_h.set(rh);
        self.view_h.set(view_h);
        let mut sc = self.scroll.get();
        if self.follow.replace(false) {
            if let Some(s) = self.selected {
                let (top, bottom) = (s as f32 * rh, (s + 1) as f32 * rh);
                if bottom - sc > view_h {
                    sc = bottom - view_h;
                } else if top < sc {
                    sc = top;
                }
            }
        }
        sc = sc.clamp(0.0, self.max_scroll());
        self.scroll.set(sc);

        let clip = Rect::new(rect.x + 1.0, rect.y + 1.0, rect.w - 2.0, view_h);
        out.push(DrawCmd::ClipPush(clip));
        let first = (sc / rh).floor() as usize;
        let last = (((sc + view_h) / rh).ceil() as usize).min(self.items.len());
        let color = if state.disabled() { p.text_disabled } else { p.text };
        let pad = t.space(10.0);
        for i in first..last {
            let y = clip.y + i as f32 * rh - sc;
            if self.selected == Some(i) {
                out.push(DrawCmd::fill(Rect::new(clip.x, y, clip.w, rh), p.selection));
            }
            let l = layout(&env.fonts, &self.items[i], &text_style(env, None, false));
            let origin = Point::new(clip.x + pad, y + (rh - l.height) / 2.0);
            for run in l.to_runs(origin, color) {
                out.push(DrawCmd::Text(run));
            }
        }
        let max = self.max_scroll();
        if max > 0.0 {
            let total = self.items.len() as f32 * rh;
            let bar_h = (view_h * view_h / total).max(t.space(16.0)).min(view_h);
            let bar_y = clip.y + (view_h - bar_h) * sc / max;
            let w = t.space(3.0);
            out.push(DrawCmd::fill(Rect::new(clip.x + clip.w - w - 1.0, bar_y, w, bar_h), p.line));
        }
        out.push(DrawCmd::ClipPop);
    }
    fn focusable(&self) -> bool {
        true
    }
    fn click(&mut self, _env: &Env, rect: Rect, pos: Point, sink: &MsgSink<Msg>) {
        let i = ((pos.y - rect.y - 1.0 + self.scroll.get()) / self.row_h.get()).floor();
        if i >= 0.0 && (i as usize) < self.items.len() {
            self.select(i as usize, sink);
        }
    }
    fn wheel(&mut self, _env: &Env, _rect: Rect, _dx: f32, dy: f32) -> bool {
        let old = self.scroll.get();
        let new = (old - dy).clamp(0.0, self.max_scroll());
        self.scroll.set(new);
        new != old
    }
    fn key(&mut self, code: &str, _mods: Modifiers, sink: &MsgSink<Msg>) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let last = self.items.len() - 1;
        let page = ((self.view_h.get() / self.row_h.get()).floor() as usize).max(2) - 1;
        let cur = self.selected;
        let to = match code {
            "ArrowDown" => cur.map_or(0, |c| (c + 1).min(last)),
            "ArrowUp" => cur.map_or(0, |c| c.saturating_sub(1)),
            "PageDown" => cur.map_or(0, |c| (c + page).min(last)),
            "PageUp" => cur.map_or(0, |c| c.saturating_sub(page)),
            "Home" => 0,
            "End" => last,
            _ => return false,
        };
        self.select(to, sink);
        true
    }
}
