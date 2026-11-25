// Copyright (c) Vitaly Golik (mjojo)

use crate::ui::{Env, Ui, Widget};
use std::cell::Cell;
use hey_core::{Callback, Modifiers, MsgSink, NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::Style;
use hey_style::{Variant, WidgetState};
use hey_text::{layout, measure, MemoryClipboard, TextEditState, TextStyle};

pub(crate) fn text_style(env: &Env, width: Option<f32>, wrap: bool) -> TextStyle {
    let mut st = TextStyle::new(env.chain.clone(), env.font_px);
    st.max_width = width;
    st.wrap = wrap;
    st
}

/// Статический текст.
pub struct Label {
    text: String,
}

impl Label {
    pub fn add<Msg: 'static>(ui: &mut Ui<Msg>, parent: NodeId, text: &str) -> NodeId {
        let env = ui.env.clone();
        let t = text.to_string();
        let measure = move |w: Option<f32>| {
            let l = layout(&env.fonts, &t, &text_style(&env, w, true));
            Size::new(l.width.ceil(), l.height.ceil())
        };
        ui.add_widget(parent, Style::leaf(), Label { text: text.into() }, measure)
    }
}

impl<Msg> Widget<Msg> for Label {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let color = if state.disabled() { env.theme.palette.text_disabled } else { env.theme.palette.text };
        let l = layout(&env.fonts, &self.text, &text_style(env, Some(rect.w), true));
        for run in l.to_runs(Point::new(rect.x, rect.y), color) {
            out.push(DrawCmd::Text(run));
        }
    }
}

/// Кнопка с подписью; по активации (клик, Enter, Space) шлёт сообщение.
pub struct Button<Msg> {
    label: String,
    variant: Variant,
    on_click: Callback<Msg, ()>,
}

impl<Msg: 'static> Button<Msg> {
    pub fn add(
        ui: &mut Ui<Msg>,
        parent: NodeId,
        label: &str,
        variant: Variant,
        on_click: Callback<Msg, ()>,
    ) -> NodeId {
        let env = ui.env.clone();
        let t = label.to_string();
        let measure = move |_w: Option<f32>| {
            let l = layout(&env.fonts, &t, &text_style(&env, None, false));
            Size::new(
                l.width.ceil() + 2.0 * env.theme.space(14.0),
                l.height.ceil() + 2.0 * env.theme.space(7.0),
            )
        };
        ui.add_widget(parent, Style::leaf(), Button { label: label.into(), variant, on_click }, measure)
    }
}

impl<Msg> Widget<Msg> for Button<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let t = &env.theme;
        out.push(DrawCmd::Rect {
            rect,
            fill: t.button_bg(self.variant, state),
            border: (self.variant == Variant::Secondary).then_some(Border { width: 1.0, color: t.palette.line }),
            chamfer: t.chamfer_small(),
            shadow: None,
        });
        let l = layout(&env.fonts, &self.label, &text_style(env, None, false));
        let origin = Point::new(rect.x + (rect.w - l.width) / 2.0, rect.y + (rect.h - l.height) / 2.0);
        for run in l.to_runs(origin, t.button_fg(self.variant, state)) {
            out.push(DrawCmd::Text(run));
        }
    }
    fn focusable(&self) -> bool {
        true
    }
    fn activate(&mut self, sink: &MsgSink<Msg>) {
        self.on_click.emit(sink, ());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    /// Обычная панель.
    Panel,
    /// Поле документа (светлее/глубже панели).
    Document,
    /// Рамка окна: крупная фаска.
    Window,
}

/// Панель/рамка окна: фон + рамка. Ширина рамки добавляется к padding,
/// поэтому контент не перекрывает рамку.
pub struct Panel {
    kind: PanelKind,
    /// Без верхней рамки: панель стыкуется с полосой `Tabs`, её базовая линия заменяет границу.
    open_top: bool,
}

impl Panel {
    pub const BORDER: f32 = 1.0;

    pub fn add<Msg: 'static>(ui: &mut Ui<Msg>, parent: NodeId, mut style: Style, kind: PanelKind) -> NodeId {
        let b = Self::BORDER;
        let p = &mut style.padding;
        p.left += b;
        p.top += b;
        p.right += b;
        p.bottom += b;
        ui.add_container_widget(parent, style, Panel { kind, open_top: false })
    }

    /// Панель под полосой `Tabs` (без верхней рамки).
    pub fn add_attached<Msg: 'static>(ui: &mut Ui<Msg>, parent: NodeId, style: Style, kind: PanelKind) -> NodeId {
        let id = Self::add(ui, parent, style, kind);
        ui.replace_widget(id, Panel { kind, open_top: true });
        id
    }
}

impl<Msg> Widget<Msg> for Panel {
    fn paint(&self, env: &Env, rect: Rect, _state: WidgetState, out: &mut DisplayList) {
        let t = &env.theme;
        let (fill, chamfer) = match self.kind {
            PanelKind::Panel => (t.palette.panel, t.chamfer_medium()),
            PanelKind::Document => (t.palette.document, 0.0),
            PanelKind::Window => (t.palette.panel, t.chamfer_large()),
        };
        let line = t.palette.line;
        if !self.open_top {
            out.push(DrawCmd::Rect {
                rect,
                fill,
                border: Some(Border { width: Self::BORDER, color: line }),
                chamfer,
                shadow: None,
            });
            return;
        }
        out.push(DrawCmd::fill(rect, fill));
        let h = Self::BORDER / 2.0;
        let (l, r, b) = (rect.x + h, rect.right() - h, rect.bottom() - h);
        for (a, c) in [
            (Point::new(l, rect.y), Point::new(l, b)),
            (Point::new(l, b), Point::new(r, b)),
            (Point::new(r, b), Point::new(r, rect.y)),
        ] {
            out.push(DrawCmd::Line { a, b: c, width: Self::BORDER, color: line });
        }
    }
}

/// Полоса вкладок. Активная вкладка сливается с панелью под ней (`Panel::add_attached`):
/// базовая линия полосы прерывается под активной вкладкой. ←/→/Home/End меняют вкладку.
pub struct Tabs<Msg> {
    labels: Vec<String>,
    active: usize,
    on_change: Callback<Msg, usize>,
}

fn tab_widths(env: &Env, labels: &[String]) -> Vec<f32> {
    labels
        .iter()
        .map(|s| layout(&env.fonts, s, &text_style(env, None, false)).width.ceil() + 2.0 * env.theme.space(14.0))
        .collect()
}

impl<Msg: 'static> Tabs<Msg> {
    pub fn add(
        ui: &mut Ui<Msg>,
        parent: NodeId,
        labels: &[&str],
        active: usize,
        on_change: Callback<Msg, usize>,
    ) -> NodeId {
        let env = ui.env.clone();
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        let l2 = labels.clone();
        let measure = move |_w: Option<f32>| {
            let h = layout(&env.fonts, "Ag", &text_style(&env, None, false)).height.ceil() + 2.0 * env.theme.space(7.0);
            Size::new(tab_widths(&env, &l2).iter().sum(), h)
        };
        let active = active.min(labels.len().saturating_sub(1));
        ui.add_widget(parent, Style::leaf(), Tabs { labels, active, on_change }, measure)
    }

    fn select(&mut self, i: usize, sink: &MsgSink<Msg>) {
        if i != self.active && i < self.labels.len() {
            self.active = i;
            self.on_change.emit(sink, i);
        }
    }
}

impl<Msg: 'static> Widget<Msg> for Tabs<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let p = &env.theme.palette;
        let widths = tab_widths(env, &self.labels);
        let (top, bottom) = (rect.y + 0.5, rect.bottom() - 0.5);
        let mut x = rect.x;
        let mut gap = (rect.x, rect.x);
        for (i, (label, w)) in self.labels.iter().zip(&widths).enumerate() {
            let active = i == self.active;
            let tab = Rect::new(x, rect.y, *w, rect.h);
            out.push(DrawCmd::fill(tab, if active { p.panel } else { p.tab_inactive }));
            let l = layout(&env.fonts, label, &text_style(env, None, false));
            let color = if active { p.text } else { p.text_muted };
            let origin = Point::new(x + (w - l.width) / 2.0, rect.y + (rect.h - l.height) / 2.0);
            for run in l.to_runs(origin, color) {
                out.push(DrawCmd::Text(run));
            }
            let (l_, r_) = (x + 0.5, x + w - 0.5);
            for (a, b) in [
                (Point::new(l_, bottom), Point::new(l_, top)),
                (Point::new(l_, top), Point::new(r_, top)),
                (Point::new(r_, top), Point::new(r_, bottom)),
            ] {
                out.push(DrawCmd::Line { a, b, width: 1.0, color: p.line });
            }
            if active {
                gap = (x + 1.0, x + w - 1.0);
                if state.focused() {
                    let f = Rect::new(x + 3.0, rect.y + 3.0, w - 6.0, rect.h - 6.0);
                    out.push(DrawCmd::Rect {
                        rect: f,
                        fill: hey_core::Color::TRANSPARENT,
                        border: Some(Border { width: env.theme.focus_width, color: p.focus }),
                        chamfer: 0.0,
                        shadow: None,
                    });
                }
            }
            x += w;
        }
        // Базовая линия: слева и справа от активной вкладки.
        let y = rect.bottom() - 0.5;
        for (a, b) in [(rect.x, gap.0), (gap.1, rect.right().max(x))] {
            if b > a {
                out.push(DrawCmd::Line { a: Point::new(a, y), b: Point::new(b, y), width: 1.0, color: p.line });
            }
        }
    }
    fn focusable(&self) -> bool {
        true
    }
    fn click(&mut self, env: &Env, rect: Rect, pos: Point, sink: &MsgSink<Msg>) {
        let mut x = rect.x;
        for (i, w) in tab_widths(env, &self.labels).iter().enumerate() {
            if pos.x >= x && pos.x < x + w {
                self.select(i, sink);
                return;
            }
            x += w;
        }
    }
    fn key(&mut self, code: &str, _mods: Modifiers, sink: &MsgSink<Msg>) -> bool {
        let n = self.labels.len();
        if n == 0 {
            return false;
        }
        let i = match code {
            "ArrowLeft" => self.active.saturating_sub(1),
            "ArrowRight" => (self.active + 1).min(n - 1),
            "Home" => 0,
            "End" => n - 1,
            _ => return false,
        };
        self.select(i, sink);
        true
    }
}

/// Однострочное поле ввода на `TextEditState`: каретка, выделение (Shift+стрелки),
/// Ctrl+A/C/X/V/Z/Y, слова (Ctrl+←/→), Backspace/Delete. Буфер обмена пока внутренний.
/// Мышь: клик, перетаскивание (выделение), двойной клик — слово, тройной — всё.
/// Esc снимает выделение. Горизонтальная прокрутка с автоследованием за кареткой.
pub struct TextInput<Msg> {
    edit: TextEditState,
    clipboard: MemoryClipboard,
    on_change: Callback<Msg, String>,
    scroll: Cell<f32>,
    follow: Cell<bool>,
}

impl<Msg: 'static> TextInput<Msg> {
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, text: &str, width: f32, on_change: Callback<Msg, String>) -> NodeId {
        let env = ui.env.clone();
        let measure_fn = move |_w: Option<f32>| {
            let h = layout(&env.fonts, "Ag", &text_style(&env, None, false)).height.ceil();
            Size::new(width, h + 2.0 * env.theme.space(7.0))
        };
        let w = TextInput {
            edit: TextEditState::new(text),
            clipboard: MemoryClipboard::default(),
            on_change,
            scroll: Cell::new(0.0),
            follow: Cell::new(true),
        };
        ui.add_widget(parent, Style::leaf(), w, measure_fn)
    }

    fn pad(env: &Env) -> f32 {
        env.theme.space(10.0)
    }

    fn x_of(&self, env: &Env, byte: usize) -> f32 {
        measure(&env.fonts, &self.edit.text()[..byte], &env.chain, env.font_px)
    }

    fn changed(&self, before: &str, sink: &MsgSink<Msg>) {
        self.follow.set(true);
        if self.edit.text() != before {
            self.on_change.emit(sink, self.edit.text().to_string());
        }
    }

    /// Байтовая позиция ближайшей границы символа к точке `pos` (с учётом прокрутки).
    fn byte_at(&self, env: &Env, rect: Rect, pos: Point) -> usize {
        let rel = pos.x - rect.x - Self::pad(env) + self.scroll.get();
        let text = self.edit.text();
        let mut best = (0usize, f32::MAX);
        for i in text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())) {
            let d = (self.x_of(env, i) - rel).abs();
            if d < best.1 {
                best = (i, d);
            }
        }
        best.0
    }
}

impl<Msg: 'static> Widget<Msg> for TextInput<Msg> {
    fn save_state(&self) -> Option<Box<dyn std::any::Any>> {
        Some(Box::new((self.scroll.get(), self.edit.anchor(), self.edit.caret())))
    }
    fn load_state(&mut self, state: Box<dyn std::any::Any>) {
        if let Ok(s) = state.downcast::<(f32, usize, usize)>() {
            let (scroll, anchor, caret) = *s;
            let t = self.edit.text();
            if anchor <= t.len() && caret <= t.len() && t.is_char_boundary(anchor) && t.is_char_boundary(caret) {
                self.edit.set_caret(anchor, false);
                self.edit.set_caret(caret, true);
            }
            self.scroll.set(scroll);
            self.follow.set(false);
        }
    }
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        let t = &env.theme;
        let p = &t.palette;
        let border = if state.focused() { p.focus } else { p.line };
        out.push(DrawCmd::Rect {
            rect,
            fill: if state.disabled() { p.panel } else { p.document },
            border: Some(Border { width: 1.0, color: border }),
            chamfer: t.chamfer_small(),
            shadow: None,
        });
        let l = layout(&env.fonts, self.edit.text(), &text_style(env, None, false));
        let inner = (rect.w - 2.0 * Self::pad(env)).max(1.0);
        let caret_x = self.x_of(env, self.edit.caret());
        let mut sc = self.scroll.get();
        if self.follow.replace(false) {
            if caret_x - sc > inner {
                sc = caret_x - inner;
            } else if caret_x < sc {
                sc = caret_x;
            }
        }
        sc = sc.clamp(0.0, (l.width + 1.0 - inner).max(0.0));
        self.scroll.set(sc);
        out.push(DrawCmd::ClipPush(Rect::new(rect.x + Self::pad(env) - 1.0, rect.y, inner + 2.0, rect.h)));
        let origin = Point::new(rect.x + Self::pad(env) - sc, rect.y + (rect.h - l.height) / 2.0);
        if self.edit.has_selection() && state.focused() {
            let r = self.edit.selection();
            let (a, b) = (self.x_of(env, r.start), self.x_of(env, r.end));
            out.push(DrawCmd::fill(Rect::new(origin.x + a, origin.y, b - a, l.height), p.selection));
        }
        let color = if state.disabled() { p.text_disabled } else { p.text };
        for run in l.to_runs(origin, color) {
            out.push(DrawCmd::Text(run));
        }
        if state.focused() {
            let x = (origin.x + caret_x).floor() + 0.5;
            out.push(DrawCmd::Line {
                a: Point::new(x, origin.y),
                b: Point::new(x, origin.y + l.height),
                width: 1.0,
                color: p.text,
            });
        }
        out.push(DrawCmd::ClipPop);
    }
    fn focusable(&self) -> bool {
        true
    }
    fn click(&mut self, _env: &Env, _rect: Rect, _pos: Point, _sink: &MsgSink<Msg>) {}
    fn press(&mut self, env: &Env, rect: Rect, pos: Point, clicks: u32, _sink: &MsgSink<Msg>) {
        let b = self.byte_at(env, rect, pos);
        match clicks {
            1 => self.edit.set_caret(b, false),
            2 => self.edit.select_word_at(b),
            _ => self.edit.select_all(),
        }
        self.follow.set(true);
    }
    fn drag(&mut self, env: &Env, rect: Rect, pos: Point, _sink: &MsgSink<Msg>) {
        let b = self.byte_at(env, rect, pos);
        self.edit.set_caret(b, true);
        self.follow.set(true);
    }
    fn key(&mut self, code: &str, mods: Modifiers, sink: &MsgSink<Msg>) -> bool {
        let before = self.edit.text().to_string();
        let (sh, ctrl) = (mods.shift, mods.ctrl);
        let e = &mut self.edit;
        match (code, ctrl) {
            ("ArrowLeft", false) => e.move_left(sh),
            ("ArrowLeft", true) => e.move_word_left(sh),
            ("ArrowRight", false) => e.move_right(sh),
            ("ArrowRight", true) => e.move_word_right(sh),
            ("Home", _) => e.home(sh),
            ("End", _) => e.end(sh),
            ("Backspace", false) => e.backspace(),
            ("Backspace", true) => e.delete_word_left(),
            ("Delete", false) => e.delete(),
            ("Delete", true) => e.delete_word_right(),
            ("Escape", _) if e.has_selection() => e.set_caret(e.caret(), false),
            ("a", true) => e.select_all(),
            ("c", true) => e.copy(&mut self.clipboard),
            ("x", true) => e.cut(&mut self.clipboard),
            ("v", true) => e.paste(&mut self.clipboard),
            ("z", true) => {
                e.undo();
            }
            ("y", true) => {
                e.redo();
            }
            _ => return false,
        }
        self.changed(&before, sink);
        true
    }
    fn text(&mut self, text: &str, sink: &MsgSink<Msg>) -> bool {
        let before = self.edit.text().to_string();
        self.edit.insert(text);
        self.changed(&before, sink);
        true
    }
}
