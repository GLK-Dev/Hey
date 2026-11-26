// Copyright (c) Vitaly Golik (mjojo)

use crate::cmd::DrawCmd;
use hey_core::Rect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListError {
    /// `ClipPop` without a matching `ClipPush` at position `index`.
    UnmatchedPop { index: usize },
    /// Unclosed `ClipPush` left at the end of the list.
    UnclosedPush { count: usize },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    cmds: Vec<DrawCmd>,
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, cmd: DrawCmd) {
        self.cmds.push(cmd);
    }

    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    /// Multiplies every coordinate, size, radius and font size by `s` (logical to physical pixels).
    pub fn scale_by(&mut self, s: f32) {
        if s == 1.0 {
            return;
        }
        let rect = |r: &mut Rect| *r = Rect::new(r.x * s, r.y * s, r.w * s, r.h * s);
        for c in &mut self.cmds {
            match c {
                DrawCmd::Rect { rect: r, border, chamfer, shadow, .. } => {
                    rect(r);
                    *chamfer *= s;
                    if let Some(b) = border {
                        b.width *= s;
                    }
                    if let Some(sh) = shadow {
                        sh.offset = hey_core::Point::new(sh.offset.x * s, sh.offset.y * s);
                        sh.blur *= s;
                    }
                }
                DrawCmd::Line { a, b, width, .. } => {
                    *a = hey_core::Point::new(a.x * s, a.y * s);
                    *b = hey_core::Point::new(b.x * s, b.y * s);
                    *width *= s;
                }
                DrawCmd::Text(run) => {
                    run.size_px *= s;
                    for g in &mut run.glyphs {
                        g.x *= s;
                        g.y *= s;
                    }
                }
                DrawCmd::Image { rect: r, .. } | DrawCmd::ClipPush(r) => rect(r),
                _ => {}
            }
        }
    }

    /// Interpolates two lists of the same shape (same command kinds in the same order): colors blend by `t`.
    /// If the shapes differ the target list wins.
    pub fn blend(from: &DisplayList, to: &DisplayList, t: f32) -> DisplayList {
        let t = t.clamp(0.0, 1.0);
        let same = from.cmds.len() == to.cmds.len()
            && from.cmds.iter().zip(&to.cmds).all(|(a, b)| match (a, b) {
                (DrawCmd::Rect { .. }, DrawCmd::Rect { .. }) | (DrawCmd::Line { .. }, DrawCmd::Line { .. }) => true,
                (DrawCmd::Text(x), DrawCmd::Text(y)) => x.glyphs.len() == y.glyphs.len(),
                (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b) && a == b,
            });
        if !same {
            return to.clone();
        }
        let cmds = from
            .cmds
            .iter()
            .zip(&to.cmds)
            .map(|(a, b)| match (a, b) {
                (DrawCmd::Rect { fill: f0, border: b0, .. }, DrawCmd::Rect { rect, fill, border, chamfer, shadow }) => DrawCmd::Rect {
                    rect: *rect,
                    fill: f0.lerp(*fill, t),
                    border: match (b0, border) {
                        (Some(x), Some(y)) => Some(crate::cmd::Border { width: y.width, color: x.color.lerp(y.color, t) }),
                        _ => *border,
                    },
                    chamfer: *chamfer,
                    shadow: *shadow,
                },
                (DrawCmd::Line { color: c0, .. }, DrawCmd::Line { a, b, width, color }) => {
                    DrawCmd::Line { a: *a, b: *b, width: *width, color: c0.lerp(*color, t) }
                }
                (DrawCmd::Text(x), DrawCmd::Text(y)) => {
                    DrawCmd::Text(crate::cmd::GlyphRun { color: x.color.lerp(y.color, t), ..y.clone() })
                }
                (_, b) => b.clone(),
            })
            .collect();
        DisplayList { cmds }
    }

    pub fn cmds(&self) -> &[DrawCmd] {
        &self.cmds
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// Checks that `ClipPush`/`ClipPop` are balanced.
    pub fn validate(&self) -> Result<(), ListError> {
        let mut depth = 0usize;
        for (index, c) in self.cmds.iter().enumerate() {
            match c {
                DrawCmd::ClipPush(_) => depth += 1,
                DrawCmd::ClipPop => {
                    depth = depth.checked_sub(1).ok_or(ListError::UnmatchedPop { index })?;
                }
                _ => {}
            }
        }
        if depth == 0 {
            Ok(())
        } else {
            Err(ListError::UnclosedPush { count: depth })
        }
    }

    /// Drawing commands with the effective clip rectangle (intersection of the whole stack).
    /// `None` means no clipping; a fully clipped area is expressed as `Some` with zero area.
    /// `ClipPush`/`ClipPop` themselves are not yielded. The list must be valid.
    pub fn iter_clipped(&self) -> impl Iterator<Item = (&DrawCmd, Option<Rect>)> {
        let mut stack: Vec<Option<Rect>> = Vec::new();
        let mut cur: Option<Rect> = None;
        self.cmds.iter().filter_map(move |c| match c {
            DrawCmd::ClipPush(r) => {
                stack.push(cur);
                cur = Some(match cur {
                    None => *r,
                    Some(prev) => prev.intersect(r).unwrap_or(Rect::new(prev.x, prev.y, 0.0, 0.0)),
                });
                None
            }
            DrawCmd::ClipPop => {
                cur = stack.pop().unwrap_or(None);
                None
            }
            other => Some((other, cur)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hey_core::Color;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn validate_detects_problems() {
        let mut l = DisplayList::new();
        l.push(DrawCmd::ClipPop);
        assert_eq!(l.validate(), Err(ListError::UnmatchedPop { index: 0 }));
        l.clear();
        l.push(DrawCmd::ClipPush(r(0.0, 0.0, 1.0, 1.0)));
        assert_eq!(l.validate(), Err(ListError::UnclosedPush { count: 1 }));
        l.push(DrawCmd::ClipPop);
        assert_eq!(l.validate(), Ok(()));
    }

    #[test]
    fn nested_clips_intersect_and_restore() {
        let mut l = DisplayList::new();
        let c = Color::hex(0xFFFFFF);
        l.push(DrawCmd::fill(r(0.0, 0.0, 1.0, 1.0), c));
        l.push(DrawCmd::ClipPush(r(0.0, 0.0, 10.0, 10.0)));
        l.push(DrawCmd::ClipPush(r(5.0, 5.0, 10.0, 10.0)));
        l.push(DrawCmd::fill(r(0.0, 0.0, 1.0, 1.0), c));
        l.push(DrawCmd::ClipPop);
        l.push(DrawCmd::fill(r(0.0, 0.0, 1.0, 1.0), c));
        l.push(DrawCmd::ClipPop);
        let clips: Vec<_> = l.iter_clipped().map(|(_, c)| c).collect();
        assert_eq!(clips, [None, Some(r(5.0, 5.0, 5.0, 5.0)), Some(r(0.0, 0.0, 10.0, 10.0))]);
    }

    #[test]
    fn disjoint_clip_is_empty() {
        let mut l = DisplayList::new();
        l.push(DrawCmd::ClipPush(r(0.0, 0.0, 2.0, 2.0)));
        l.push(DrawCmd::ClipPush(r(5.0, 5.0, 2.0, 2.0)));
        l.push(DrawCmd::fill(r(0.0, 0.0, 9.0, 9.0), Color::hex(0)));
        let (_, clip) = l.iter_clipped().next().unwrap();
        let clip = clip.unwrap();
        assert_eq!((clip.w, clip.h), (0.0, 0.0));
    }
}
