// Copyright (c) Vitaly Golik (mjojo)

//! Layout lints: content that spills out of its container or collides with a sibling.

use hey_core::{NodeId, Rect};
use hey_layout::LayoutTree;
use std::fmt;

/// Rectangles closer than this (logical px) are treated as touching, not overlapping.
const EPS: f32 = 0.5;

/// What a layout lint found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayoutIssueKind {
    /// A child extends past its parent, which neither scrolls nor grows to fit.
    Clipped,
    /// Two siblings cover the same area.
    Overlap,
}

/// One finding, tied to a node (and the sibling it collides with, for overlaps).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutIssue {
    pub kind: LayoutIssueKind,
    pub node: NodeId,
    /// The other sibling for [`LayoutIssueKind::Overlap`].
    pub other: Option<NodeId>,
    pub rect: Rect,
}

impl fmt::Display for LayoutIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.rect;
        match self.kind {
            LayoutIssueKind::Clipped => write!(f, "node {:?} at ({}, {}, {}x{}) is clipped by its parent", self.node, r.x, r.y, r.w, r.h),
            LayoutIssueKind::Overlap => {
                write!(f, "node {:?} at ({}, {}, {}x{}) overlaps sibling {:?}", self.node, r.x, r.y, r.w, r.h, self.other)
            }
        }
    }
}

/// Walks a computed layout tree (logical pixels). Popup, hidden and scrolling nodes are exempt.
pub fn audit_layout(tree: &LayoutTree) -> Vec<LayoutIssue> {
    let mut out = Vec::new();
    walk(tree, tree.root(), &mut out);
    out
}

fn visible(tree: &LayoutTree, id: NodeId) -> bool {
    tree.style(id).is_some_and(|s| !s.hidden && !s.popup)
}

fn walk(tree: &LayoutTree, id: NodeId, out: &mut Vec<LayoutIssue>) {
    let scrolls = tree.style(id).is_some_and(|s| s.scroll_x || s.scroll_y);
    let rect = tree.rect(id);
    let kids: Vec<NodeId> = tree.children(id).iter().copied().filter(|&c| visible(tree, c)).collect();
    for (i, &c) in kids.iter().enumerate() {
        let r = tree.rect(c);
        if r.w <= 0.0 || r.h <= 0.0 {
            continue;
        }
        if !scrolls && (r.x < rect.x - EPS || r.y < rect.y - EPS || r.right() > rect.right() + EPS || r.bottom() > rect.bottom() + EPS) {
            out.push(LayoutIssue { kind: LayoutIssueKind::Clipped, node: c, other: None, rect: r });
        }
        for &d in &kids[i + 1..] {
            let s = tree.rect(d);
            if s.w <= 0.0 || s.h <= 0.0 || tree.style(c).map(|x| x.z) != tree.style(d).map(|x| x.z) {
                continue;
            }
            let (ox, oy) = (r.right().min(s.right()) - r.x.max(s.x), r.bottom().min(s.bottom()) - r.y.max(s.y));
            if ox > EPS && oy > EPS {
                out.push(LayoutIssue { kind: LayoutIssueKind::Overlap, node: c, other: Some(d), rect: r });
            }
        }
        walk(tree, c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hey_core::Size;
    use hey_layout::Style;

    #[test]
    fn flags_spill_and_collision_but_not_clean_layouts() {
        let mut t = LayoutTree::new(Style::column().size(100.0, 100.0));
        let root = t.root();
        t.add(root, Style::leaf().size(40.0, 60.0).shrink(0.0));
        t.add(root, Style::leaf().size(40.0, 60.0).shrink(0.0));
        t.compute(Size::new(100.0, 100.0));
        let issues = audit_layout(&t);
        assert!(issues.iter().any(|i| i.kind == LayoutIssueKind::Clipped), "{issues:?}");

        let mut t = LayoutTree::new(Style::column().size(100.0, 200.0));
        let root = t.root();
        t.add(root, Style::leaf().size(40.0, 60.0));
        t.add(root, Style::leaf().size(40.0, 60.0));
        t.compute(Size::new(100.0, 200.0));
        assert!(audit_layout(&t).is_empty());
    }
}
