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
        ui.add_container_widget(parent, style, Panel { kind })
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
        out.push(DrawCmd::Rect {
            rect,
            fill,
            border: Some(Border { width: Self::BORDER, color: t.palette.line }),
            chamfer,
            shadow: None,
        });
    }
}
