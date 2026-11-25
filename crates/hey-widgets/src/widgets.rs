// Copyright (c) Vitaly Golik (mjojo)

use crate::ui::{Env, Ui, Widget};
use hey_core::{Callback, MsgSink, NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::Style;
use hey_style::{Variant, WidgetState};
use hey_text::{layout, TextStyle};

fn text_style(env: &Env, width: Option<f32>, wrap: bool) -> TextStyle {
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
    fn key(&mut self, code: &str, sink: &MsgSink<Msg>) -> bool {
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
