// Copyright (c) Vitaly Golik (mjojo)

use crate::ui::{Env, Ui, Widget};
use crate::widgets::text_style;
use hey_core::{Callback, Modifiers, MsgSink, NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};
use hey_layout::Style;
use hey_style::WidgetState;
use hey_text::{layout, Layout, MemoryClipboard, TextEditState};
use std::cell::Cell;
use std::rc::Rc;

/// Раскладка текста с таблицей начал строк (байтовые смещения).
struct Geo {
    l: Layout,
    starts: Vec<usize>,
    lh: f32,
}

impl Geo {
    fn new(env: &Env, text: &str, inner_w: f32) -> Geo {
        let l = layout(&env.fonts, text, &text_style(env, Some(inner_w), true));
        let mut para_starts = vec![0usize];
        para_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        let para_of = |b: usize| para_starts.partition_point(|&s| s <= b) - 1;
        let mut starts = Vec::with_capacity(l.lines.len());
        let mut last: Option<usize> = None;
        for line in &l.lines {
            let (para, start) = match line.glyphs.first() {
                Some(g) => (para_of(g.byte), g.byte),
                None => {
                    let p = last.map_or(0, |p| p + 1).min(para_starts.len() - 1);
                    (p, para_starts[p])
                }
            };
            last = Some(para);
            starts.push(start);
        }
        let lh = l.height / l.lines.len().max(1) as f32;
        Geo { l, starts, lh }
    }

    fn line_of(&self, byte: usize) -> usize {
        self.starts.partition_point(|&s| s <= byte).saturating_sub(1)
    }

    /// Конец строки в тексте (начало следующей или конец текста).
    fn line_end(&self, i: usize, len: usize) -> usize {
        self.starts.get(i + 1).copied().unwrap_or(len)
    }

    fn x_of(&self, line: usize, byte: usize) -> f32 {
        let ln = &self.l.lines[line];
        ln.glyphs.iter().find(|g| g.byte >= byte).map_or(ln.width, |g| g.x)
    }

    fn byte_at(&self, text: &str, line: usize, x: f32) -> usize {
        let ln = &self.l.lines[line];
        for g in &ln.glyphs {
            if x < g.x + g.advance / 2.0 {
                return g.byte;
            }
        }
        match ln.glyphs.last() {
            Some(g) => g.byte + text[g.byte..].chars().next().map_or(0, char::len_utf8),
            None => self.starts[line],
        }
    }

    fn line_at_y(&self, y: f32) -> usize {
        ((y / self.lh).floor().max(0.0) as usize).min(self.l.lines.len() - 1)
    }
}

/// Многострочное поле ввода: перенос по ширине, Enter, Up/Down/PageUp/PageDown,
/// вертикальная прокрутка (колесо и автоследование за кареткой), выделение мышью.
pub struct TextArea<Msg> {
    env: Rc<Env>,
    edit: TextEditState,
    clipboard: MemoryClipboard,
    on_change: Callback<Msg, String>,
    scroll: Cell<f32>,
    follow: Cell<bool>,
    /// Размер прямоугольника при последней отрисовке (для клавиш без `rect`).
    size: Cell<(f32, f32)>,
}

impl<Msg: 'static> TextArea<Msg> {
    /// `rows` — высота в строках.
    pub fn add(ui: &mut Ui<Msg>, parent: NodeId, text: &str, width: f32, rows: usize, on_change: Callback<Msg, String>) -> NodeId {
        let env = ui.env.clone();
        let e2 = env.clone();
        let measure_fn = move |_w: Option<f32>| {
            let lh = layout(&e2.fonts, "Ag", &text_style(&e2, None, false)).height;
            Size::new(width, (lh * rows as f32).ceil() + 2.0 * Self::vpad(&e2))
        };
        let h = measure_fn(None).h;
        let w = TextArea {
            env,
            edit: TextEditState::new(text),
            clipboard: MemoryClipboard::default(),
            on_change,
            scroll: Cell::new(0.0),
            follow: Cell::new(true),
            size: Cell::new((width, h)),
        };
        ui.add_widget(parent, Style::leaf(), w, measure_fn)
    }

    fn pad(env: &Env) -> f32 {
        env.theme.space(10.0)
    }
    fn vpad(env: &Env) -> f32 {
        env.theme.space(7.0)
    }

    fn geo(&self) -> Geo {
        let (w, _) = self.size.get();
        Geo::new(&self.env, self.edit.text(), (w - 2.0 * Self::pad(&self.env)).max(1.0))
    }

    fn changed(&self, before: &str, sink: &MsgSink<Msg>) {
        self.follow.set(true);
        if self.edit.text() != before {
            self.on_change.emit(sink, self.edit.text().to_string());
        }
    }

    fn byte_at_point(&self, rect: Rect, pos: Point) -> usize {
        self.size.set((rect.w, rect.h));
        let g = self.geo();
        let line = g.line_at_y(pos.y - rect.y - Self::vpad(&self.env) + self.scroll.get());
        g.byte_at(self.edit.text(), line, pos.x - rect.x - Self::pad(&self.env))
    }

    fn move_lines(&mut self, n: isize, extend: bool) {
        let g = self.geo();
        let c = self.edit.caret();
        let line = g.line_of(c);
        let x = g.x_of(line, c);
        let target = line as isize + n;
        let last = g.l.lines.len() as isize - 1;
        let to = if target < 0 {
            0
        } else if target > last {
            self.edit.text().len()
        } else {
            g.byte_at(self.edit.text(), target as usize, x)
        };
        self.edit.set_caret(to, extend);
    }
}

impl<Msg: 'static> Widget<Msg> for TextArea<Msg> {
    fn paint(&self, env: &Env, rect: Rect, state: WidgetState, out: &mut DisplayList) {
        self.size.set((rect.w, rect.h));
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
        let (pad, vpad) = (Self::pad(env), Self::vpad(env));
        let (inner_w, inner_h) = ((rect.w - 2.0 * pad).max(1.0), (rect.h - 2.0 * vpad).max(1.0));
        let text = self.edit.text();
        let g = Geo::new(env, text, inner_w);
        let c = self.edit.caret();
        let cl = g.line_of(c);
        let mut sc = self.scroll.get();
        if self.follow.replace(false) {
            let (top, bottom) = (cl as f32 * g.lh, (cl + 1) as f32 * g.lh);
            if bottom - sc > inner_h {
                sc = bottom - inner_h;
            } else if top < sc {
                sc = top;
            }
        }
        sc = sc.clamp(0.0, (g.l.height - inner_h).max(0.0));
        self.scroll.set(sc);

        out.push(DrawCmd::ClipPush(Rect::new(rect.x + 1.0, rect.y + vpad - 1.0, rect.w - 2.0, inner_h + 2.0)));
        let origin = Point::new(rect.x + pad, rect.y + vpad - sc);
        if self.edit.has_selection() && state.focused() {
            let sel = self.edit.selection();
            for (i, ln) in g.l.lines.iter().enumerate() {
                let (start, end) = (g.starts[i], g.line_end(i, text.len()));
                if sel.start >= end.max(start + 1) || sel.end <= start {
                    continue;
                }
                let x0 = g.x_of(i, sel.start.max(start));
                let x1 = if sel.end >= end && i + 1 < g.l.lines.len() { ln.width + env.font_px * 0.3 } else { g.x_of(i, sel.end) };
                if x1 > x0 {
                    out.push(DrawCmd::fill(Rect::new(origin.x + x0, origin.y + i as f32 * g.lh, x1 - x0, g.lh), p.selection));
                }
            }
        }
        let color = if state.disabled() { p.text_disabled } else { p.text };
        for run in g.l.to_runs(origin, color) {
            out.push(DrawCmd::Text(run));
        }
        if state.focused() {
            let x = (origin.x + g.x_of(cl, c)).floor() + 0.5;
            let y = origin.y + cl as f32 * g.lh;
            out.push(DrawCmd::Line { a: Point::new(x, y), b: Point::new(x, y + g.lh), width: 1.0, color: p.text });
        }
        out.push(DrawCmd::ClipPop);
    }
    fn focusable(&self) -> bool {
        true
    }
    fn click(&mut self, _env: &Env, _rect: Rect, _pos: Point, _sink: &MsgSink<Msg>) {}
    fn press(&mut self, _env: &Env, rect: Rect, pos: Point, clicks: u32, _sink: &MsgSink<Msg>) {
        let b = self.byte_at_point(rect, pos);
        match clicks {
            1 => self.edit.set_caret(b, false),
            2 => self.edit.select_word_at(b),
            _ => {
                let g = self.geo();
                let line = g.line_of(b);
                let end = g.line_end(line, self.edit.text().len());
                self.edit.set_caret(g.starts[line], false);
                self.edit.set_caret(end, true);
            }
        }
        self.follow.set(true);
    }
    fn drag(&mut self, _env: &Env, rect: Rect, pos: Point, _sink: &MsgSink<Msg>) {
        let b = self.byte_at_point(rect, pos);
        self.edit.set_caret(b, true);
        self.follow.set(true);
    }
    fn wheel(&mut self, env: &Env, rect: Rect, _dx: f32, dy: f32) -> bool {
        self.size.set((rect.w, rect.h));
        let inner_h = rect.h - 2.0 * Self::vpad(env);
        let max = (self.geo().l.height - inner_h).max(0.0);
        let old = self.scroll.get();
        self.scroll.set((old - dy).clamp(0.0, max));
        true
    }
    fn key(&mut self, code: &str, mods: Modifiers, sink: &MsgSink<Msg>) -> bool {
        let before = self.edit.text().to_string();
        let (sh, ctrl) = (mods.shift, mods.ctrl);
        let page = ((self.size.get().1 - 2.0 * Self::vpad(&self.env)) / self.geo().lh).floor().max(2.0) as isize - 1;
        match (code, ctrl) {
            ("ArrowUp", _) => self.move_lines(-1, sh),
            ("ArrowDown", _) => self.move_lines(1, sh),
            ("PageUp", _) => self.move_lines(-page, sh),
            ("PageDown", _) => self.move_lines(page, sh),
            ("Home", false) | ("End", false) => {
                let g = self.geo();
                let line = g.line_of(self.edit.caret());
                let to = if code == "Home" { g.starts[line] } else { g.byte_at(self.edit.text(), line, f32::MAX) };
                self.edit.set_caret(to, sh);
            }
            ("Home", true) => self.edit.home(sh),
            ("End", true) => self.edit.end(sh),
            ("ArrowLeft", false) => self.edit.move_left(sh),
            ("ArrowLeft", true) => self.edit.move_word_left(sh),
            ("ArrowRight", false) => self.edit.move_right(sh),
            ("ArrowRight", true) => self.edit.move_word_right(sh),
            ("Enter", _) => self.edit.insert("\n"),
            ("Backspace", false) => self.edit.backspace(),
            ("Backspace", true) => self.edit.delete_word_left(),
            ("Delete", false) => self.edit.delete(),
            ("Delete", true) => self.edit.delete_word_right(),
            ("Escape", _) if self.edit.has_selection() => self.edit.set_caret(self.edit.caret(), false),
            ("a", true) => self.edit.select_all(),
            ("c", true) => self.edit.copy(&mut self.clipboard),
            ("x", true) => self.edit.cut(&mut self.clipboard),
            ("v", true) => self.edit.paste(&mut self.clipboard),
            ("z", true) => {
                self.edit.undo();
            }
            ("y", true) => {
                self.edit.redo();
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
