// Copyright (c) Vitaly Golik (mjojo)

//! Display-list inspector: an optional overlay showing the widget under the pointer and a diff of
//! the frame against a pinned snapshot. Off by default; when off, `Ui` pays one `Option` check per frame.

use super::Ui;
use crate::overlay::{panel, push_text};
use hey_core::{NodeId, Point, Rect, Size};
use hey_draw::{Border, DisplayList, DrawCmd};

/// Inspector state, present only while enabled.
#[derive(Default)]
pub(crate) struct Inspector {
    /// Previous frame, without the inspector overlay.
    pub(crate) last: DisplayList,
    /// Frame pinned as the diff baseline.
    pub(crate) snapshot: Option<DisplayList>,
}

impl<Msg: 'static> Ui<Msg> {
    /// Turns the inspector overlay on or off. Turning it off drops the snapshot.
    pub fn set_inspector(&mut self, on: bool) {
        if on != self.inspector.is_some() {
            self.inspector = on.then(Inspector::default);
        }
    }

    /// Shows (or with `None` hides) a one-line status bar along the bottom edge, used by the shell's
    /// time-travel scrubber. Returns whether the text changed.
    pub fn set_note(&mut self, note: Option<String>) -> bool {
        let changed = self.note != note;
        self.note = note;
        changed
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    pub(super) fn paint_note(&mut self, viewport: Size, out: &mut DisplayList) {
        let Some(text) = &self.note else { return };
        let env = self.env.clone();
        let line_h = env.font_px * 1.4;
        let r = Rect::new(8.0, (viewport.h - line_h - 20.0).max(0.0), (viewport.w - 16.0).max(0.0), line_h + 12.0);
        panel(&env, r, out);
        push_text(&env, text, Point::new(r.x + 8.0, r.y + 6.0), env.theme.palette.text, out);
    }

    pub fn inspector_enabled(&self) -> bool {
        self.inspector.is_some()
    }

    /// Pins the last painted frame as the baseline for [`inspector_diff`](Self::inspector_diff).
    /// Returns false when the inspector is off.
    pub fn inspector_snapshot(&mut self) -> bool {
        match &mut self.inspector {
            Some(i) => {
                i.snapshot = Some(i.last.clone());
                true
            }
            None => false,
        }
    }

    /// Lines (`- old` / `+ new`) that differ between the snapshot and the last frame.
    pub fn inspector_diff(&self) -> Vec<String> {
        match &self.inspector {
            Some(Inspector { last, snapshot: Some(s) }) => s.diff(last),
            _ => Vec::new(),
        }
    }

    /// Text description of a node: role, label, rect, state, flags and layout style.
    pub fn inspect_node(&self, id: NodeId) -> Vec<String> {
        let r = self.tree.rect(id);
        let n = self.nodes.get(&id);
        let a = n.and_then(|n| n.widget.a11y());
        let mut v = vec![match &a {
            Some(a) => format!("{:?} \"{}\"", a.role, a.label),
            None => "container".to_string(),
        }];
        v.push(format!("rect {:.1},{:.1} {:.1}x{:.1}", r.x, r.y, r.w, r.h));
        let st = self.state_of(id);
        v.push(format!("state hover={} press={} focus={}", self.hover == Some(id), self.pressed == Some(id), st.focused()));
        if let Some(n) = n {
            v.push(format!("disabled={} focusable={}", n.disabled, n.widget.focusable()));
        }
        if let Some(s) = self.tree.style(id) {
            v.push(format!("{:?} grow={} shrink={} clip={} z={}", s.kind, s.grow, s.shrink, s.clip, s.z));
        }
        v
    }

    /// Indented dump of the layout tree: node index, rect and widget role.
    pub fn dump_tree(&self) -> String {
        let mut s = String::new();
        let mut stack = vec![(self.tree.root(), 0usize)];
        while let Some((id, depth)) = stack.pop() {
            let r = self.tree.rect(id);
            let role =
                self.nodes.get(&id).and_then(|n| n.widget.a11y()).map_or("-".to_string(), |a| format!("{:?} \"{}\"", a.role, a.label));
            s.push_str(&format!("{}{role} {:.1},{:.1} {:.1}x{:.1}\n", "  ".repeat(depth), r.x, r.y, r.w, r.h));
            stack.extend(self.tree.children(id).iter().rev().map(|&c| (c, depth + 1)));
        }
        s
    }

    /// Records the frame and draws the overlay on top of it (logical coordinates).
    pub(super) fn paint_inspector(&mut self, viewport: Size, out: &mut DisplayList) {
        let Some(insp) = &mut self.inspector else { return };
        insp.last = out.clone();
        let diff = insp.snapshot.as_ref().map(|s| s.diff(&insp.last).len());
        let mut lines = vec![format!("{} cmds", out.len())];
        if let Some(d) = diff {
            lines.push(format!("{d} changed vs snapshot"));
        }
        let env = self.env.clone();
        let p = &env.theme.palette;
        if let Some(id) = self.hover {
            let r = self.tree.rect(id);
            out.push(DrawCmd::Rect {
                rect: r,
                fill: hey_core::Color::rgba(p.focus.r, p.focus.g, p.focus.b, 40),
                border: Some(Border { width: 1.0, color: p.focus }),
                chamfer: 0.0,
                shadow: None,
            });
            lines.extend(self.inspect_node(id));
        }
        let line_h = env.font_px * 1.4;
        let (w, h) = (300.0, line_h * lines.len() as f32 + 12.0);
        let panel_rect = Rect::new((viewport.w - w - 8.0).max(0.0), 8.0, w, h);
        panel(&env, panel_rect, out);
        for (i, l) in lines.iter().enumerate() {
            push_text(&env, l, Point::new(panel_rect.x + 8.0, panel_rect.y + 6.0 + i as f32 * line_h), p.text, out);
        }
    }
}
