// Copyright (c) Vitaly Golik (mjojo)

//! Dockable panels: a pure-data layout (split tree of tabbed groups, plus floating roots that
//! live in their own windows) and a function that renders one root as `View`s.
//!
//! The model knows nothing about windows: a floating root is tagged with a `u32` the app
//! chooses (the same number it passes to `Command::open_window`). Dragging is left to the app;
//! it calls `Dock::move_panel` / `Dock::float` from its `update`.

use crate::view::View;
use crate::widgets::Tabs;
use hey_core::Callback;
use hey_layout::{Axis, Style};
use std::mem;

/// Application-chosen panel identifier.
pub type PanelId = u32;

/// Where a dragged panel lands relative to a target panel's group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropZone {
    /// Added as a tab of the target's group.
    Center,
    /// Split: the dragged panel goes to the left of the target group.
    Left,
    /// Split: to the right.
    Right,
    /// Split: above.
    Top,
    /// Split: below.
    Bottom,
}

/// One node of the layout tree.
#[derive(Clone, Debug, PartialEq)]
pub enum DockNode {
    /// Two children side by side (`Row`) or stacked (`Column`); `ratio` is the first child's share.
    Split { axis: Axis, ratio: f32, first: Box<DockNode>, second: Box<DockNode> },
    /// Tabbed panels, one visible.
    Group { panels: Vec<PanelId>, active: usize },
}

enum Removed {
    NotFound,
    Done,
    /// The node became empty and must be dropped by the parent.
    Empty,
}

impl DockNode {
    /// A tab group; the first panel is active.
    pub fn group(panels: impl IntoIterator<Item = PanelId>) -> Self {
        DockNode::Group { panels: panels.into_iter().collect(), active: 0 }
    }

    /// A split with the given share for `first` (clamped to 0.1..0.9).
    pub fn split(axis: Axis, ratio: f32, first: DockNode, second: DockNode) -> Self {
        DockNode::Split { axis, ratio: ratio.clamp(0.1, 0.9), first: Box::new(first), second: Box::new(second) }
    }

    /// Every panel in the subtree, in layout order.
    pub fn panels(&self) -> Vec<PanelId> {
        match self {
            DockNode::Group { panels, .. } => panels.clone(),
            DockNode::Split { first, second, .. } => {
                let mut v = first.panels();
                v.extend(second.panels());
                v
            }
        }
    }

    fn empty() -> Self {
        DockNode::Group { panels: Vec::new(), active: 0 }
    }

    fn contains(&self, p: PanelId) -> bool {
        match self {
            DockNode::Group { panels, .. } => panels.contains(&p),
            DockNode::Split { first, second, .. } => first.contains(p) || second.contains(p),
        }
    }

    fn remove(&mut self, p: PanelId) -> Removed {
        match self {
            DockNode::Group { panels, active } => {
                let Some(i) = panels.iter().position(|x| *x == p) else { return Removed::NotFound };
                panels.remove(i);
                if panels.is_empty() {
                    return Removed::Empty;
                }
                if i < *active {
                    *active -= 1;
                }
                *active = (*active).min(panels.len() - 1);
                Removed::Done
            }
            DockNode::Split { first, second, .. } => {
                match first.remove(p) {
                    Removed::Empty => {
                        let rest = mem::replace(&mut **second, DockNode::empty());
                        *self = rest;
                        return Removed::Done;
                    }
                    Removed::Done => return Removed::Done,
                    Removed::NotFound => {}
                }
                match second.remove(p) {
                    Removed::Empty => {
                        let rest = mem::replace(&mut **first, DockNode::empty());
                        *self = rest;
                        Removed::Done
                    }
                    other => other,
                }
            }
        }
    }

    /// Inserts `p` next to `target`; false if `target` is not in this subtree.
    fn insert(&mut self, target: PanelId, p: PanelId, zone: DropZone) -> bool {
        match self {
            DockNode::Group { panels, active } => {
                if !panels.contains(&target) {
                    return false;
                }
                if zone == DropZone::Center {
                    panels.push(p);
                    *active = panels.len() - 1;
                    return true;
                }
                let old = mem::replace(self, DockNode::empty());
                let new = DockNode::group([p]);
                let (axis, new_first) = match zone {
                    DropZone::Left => (Axis::Row, true),
                    DropZone::Right => (Axis::Row, false),
                    DropZone::Top => (Axis::Column, true),
                    _ => (Axis::Column, false),
                };
                *self = if new_first { DockNode::split(axis, 0.5, new, old) } else { DockNode::split(axis, 0.5, old, new) };
                true
            }
            DockNode::Split { first, second, .. } => first.insert(target, p, zone) || second.insert(target, p, zone),
        }
    }

    fn select(&mut self, p: PanelId) -> bool {
        match self {
            DockNode::Group { panels, active } => match panels.iter().position(|x| *x == p) {
                Some(i) => {
                    *active = i;
                    true
                }
                None => false,
            },
            DockNode::Split { first, second, .. } => first.select(p) || second.select(p),
        }
    }

    fn set_ratio_at(&mut self, path: &[bool], r: f32) -> bool {
        match (self, path.split_first()) {
            (DockNode::Split { ratio, .. }, None) => {
                *ratio = r.clamp(0.1, 0.9);
                true
            }
            (DockNode::Split { first, second, .. }, Some((go_second, rest))) => {
                if *go_second {
                    second.set_ratio_at(rest, r)
                } else {
                    first.set_ratio_at(rest, r)
                }
            }
            _ => false,
        }
    }
}

/// The whole docking state: the main layout and the roots floating in extra windows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dock {
    main: Option<DockNode>,
    floating: Vec<(u32, DockNode)>,
}

impl Dock {
    /// A dock whose main window shows `root`.
    pub fn new(root: DockNode) -> Self {
        Dock { main: Some(root), floating: Vec::new() }
    }

    /// The main window's layout (`None` when every panel was floated or closed).
    pub fn main(&self) -> Option<&DockNode> {
        self.main.as_ref()
    }

    /// The layout floating in window `window`.
    pub fn floating(&self, window: u32) -> Option<&DockNode> {
        self.floating.iter().find(|(w, _)| *w == window).map(|(_, n)| n)
    }

    /// Window numbers that currently hold floating panels.
    pub fn floating_windows(&self) -> Vec<u32> {
        self.floating.iter().map(|(w, _)| *w).collect()
    }

    /// Every panel, main layout first.
    pub fn panels(&self) -> Vec<PanelId> {
        let mut v = self.main.as_ref().map(DockNode::panels).unwrap_or_default();
        for (_, n) in &self.floating {
            v.extend(n.panels());
        }
        v
    }

    /// Makes `panel` the visible tab of its group.
    pub fn select(&mut self, panel: PanelId) -> bool {
        self.main.as_mut().is_some_and(|n| n.select(panel)) || self.floating.iter_mut().any(|(_, n)| n.select(panel))
    }

    /// Removes `panel` from the layout. Emptied groups collapse; an emptied floating root disappears
    /// (the app should close that window, see `Dock::floating_windows`).
    pub fn close(&mut self, panel: PanelId) -> bool {
        let mut found = false;
        if let Some(n) = &mut self.main {
            match n.remove(panel) {
                Removed::NotFound => {}
                Removed::Done => found = true,
                Removed::Empty => {
                    self.main = None;
                    found = true;
                }
            }
        }
        if !found {
            for (_, n) in &mut self.floating {
                match n.remove(panel) {
                    Removed::NotFound => {}
                    Removed::Done => {
                        found = true;
                        break;
                    }
                    Removed::Empty => {
                        *n = DockNode::empty();
                        found = true;
                        break;
                    }
                }
            }
            self.floating.retain(|(_, n)| !matches!(n, DockNode::Group { panels, .. } if panels.is_empty()));
        }
        found
    }

    /// Moves `panel` next to `target` (as a tab or a split). False, and nothing changes, if either is
    /// unknown or they are the same panel.
    pub fn move_panel(&mut self, panel: PanelId, target: PanelId, zone: DropZone) -> bool {
        if panel == target || !self.has(panel) || !self.has(target) {
            return false;
        }
        self.close(panel);
        let placed = self.main.as_mut().is_some_and(|n| n.insert(target, panel, zone))
            || self.floating.iter_mut().any(|(_, n)| n.insert(target, panel, zone));
        debug_assert!(placed, "target vanished while moving");
        true
    }

    /// Tears `panel` off into its own floating root tagged `window` (a fresh number, or one that
    /// already holds floating panels: then it becomes a tab there).
    pub fn float(&mut self, panel: PanelId, window: u32) -> bool {
        if !self.has(panel) {
            return false;
        }
        // Already alone in that window: nothing to do.
        if self.floating(window).is_some_and(|n| n.panels() == [panel]) {
            return true;
        }
        self.close(panel);
        match self.floating.iter_mut().find(|(w, _)| *w == window) {
            Some((_, n)) => {
                if let Some(first) = n.panels().first().copied() {
                    n.insert(first, panel, DropZone::Center);
                }
            }
            None => self.floating.push((window, DockNode::group([panel]))),
        }
        true
    }

    /// Puts everything floating in `window` back into the main layout (call it when the window is
    /// closed). Panels join the first group, or become the root if the main layout is empty.
    pub fn dock_back(&mut self, window: u32) -> bool {
        let Some(i) = self.floating.iter().position(|(w, _)| *w == window) else { return false };
        let (_, node) = self.floating.remove(i);
        let moved = node.panels();
        match &mut self.main {
            None => self.main = Some(DockNode::group(moved)),
            Some(root) => {
                let anchor = root.panels().first().copied();
                match anchor {
                    Some(a) => {
                        for p in moved {
                            root.insert(a, p, DropZone::Center);
                        }
                    }
                    None => *root = DockNode::group(moved),
                }
            }
        }
        true
    }

    /// Sets the split ratio at `path` (false = first child, true = second) of the main layout.
    pub fn set_ratio(&mut self, path: &[bool], ratio: f32) -> bool {
        self.main.as_mut().is_some_and(|n| n.set_ratio_at(path, ratio))
    }

    fn has(&self, p: PanelId) -> bool {
        self.main.as_ref().is_some_and(|n| n.contains(p)) || self.floating.iter().any(|(_, n)| n.contains(p))
    }
}

/// Renders one layout root. `title` names a panel's tab, `content` builds the visible panel's
/// views, `on_select` turns a tab click into a message (call `Dock::select` there).
pub fn dock_view<Msg: 'static>(
    root: &DockNode,
    title: &dyn Fn(PanelId) -> String,
    content: &dyn Fn(PanelId) -> Vec<View<Msg>>,
    on_select: &Callback<Msg, PanelId>,
) -> View<Msg> {
    node_view(root, "dock", 1.0, title, content, on_select)
}

fn node_view<Msg: 'static>(
    node: &DockNode,
    key: &str,
    grow: f32,
    title: &dyn Fn(PanelId) -> String,
    content: &dyn Fn(PanelId) -> Vec<View<Msg>>,
    on_select: &Callback<Msg, PanelId>,
) -> View<Msg> {
    match node {
        DockNode::Split { axis, ratio, first, second } => {
            let style = if *axis == Axis::Row { Style::row() } else { Style::column() }.grow(grow);
            let a = node_view(first, "a", *ratio, title, content, on_select);
            let b = node_view(second, "b", 1.0 - *ratio, title, content, on_select);
            View::container(key, style, vec![a, b])
        }
        DockNode::Group { panels, active } => {
            let active = (*active).min(panels.len().saturating_sub(1));
            let labels: Vec<String> = panels.iter().map(|p| title(*p)).collect();
            let ids = panels.clone();
            let cb = on_select.clone();
            let mut kids = Vec::new();
            kids.push(View::leaf("tabs", (labels.clone(), active), move |ui, parent| {
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                let ids = ids.clone();
                let cb = cb.clone();
                Tabs::add(ui, parent, &refs, active, Callback::new(move |i: usize| cb.call(ids[i.min(ids.len() - 1)])))
            }));
            if let Some(&p) = panels.get(active) {
                kids.push(View::container(
                    "body",
                    Style::column().grow(1.0),
                    vec![View::container(u64::from(p), Style::column().grow(1.0), content(p))],
                ));
            }
            View::container(key, Style::column().grow(grow), kids)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Dock {
        Dock::new(DockNode::split(Axis::Row, 0.3, DockNode::group([1, 2]), DockNode::group([3])))
    }

    #[test]
    fn close_collapses_split() {
        let mut d = sample();
        assert!(d.close(3));
        assert_eq!(d.main(), Some(&DockNode::group([1, 2])));
        assert!(d.close(1) && d.close(2));
        assert!(d.main().is_none());
        assert!(!d.close(9));
    }

    #[test]
    fn move_as_tab_and_as_split() {
        let mut d = sample();
        assert!(d.move_panel(3, 1, DropZone::Center));
        assert_eq!(d.main(), Some(&DockNode::Group { panels: vec![1, 2, 3], active: 2 }));
        assert!(d.move_panel(2, 1, DropZone::Bottom));
        assert_eq!(d.panels(), vec![1, 3, 2]);
        assert!(matches!(d.main(), Some(DockNode::Split { axis: Axis::Column, .. })));
        assert!(!d.move_panel(1, 1, DropZone::Left));
        assert!(!d.move_panel(1, 99, DropZone::Left));
    }

    #[test]
    fn float_and_dock_back() {
        let mut d = sample();
        assert!(d.float(2, 7));
        assert_eq!(d.floating(7), Some(&DockNode::group([2])));
        assert_eq!(d.main().unwrap().panels(), vec![1, 3]);
        // Dragging a floating panel onto a main panel moves it back; the empty window disappears.
        assert!(d.move_panel(2, 3, DropZone::Center));
        assert!(d.floating_windows().is_empty());
        // Window closed by the user: its panels return.
        assert!(d.float(1, 8));
        assert!(d.dock_back(8));
        assert_eq!(d.panels().len(), 3);
        assert!(d.floating_windows().is_empty());
        // Floating the only panel of a window is a no-op; closing the main root keeps floats.
        assert!(d.float(3, 9) && d.float(3, 9));
        assert!(d.close(1) && d.close(2));
        assert!(d.main().is_none());
        assert!(d.dock_back(9));
        assert_eq!(d.main(), Some(&DockNode::group([3])));
    }

    #[test]
    fn select_and_ratio() {
        let mut d = sample();
        assert!(d.select(2));
        assert!(matches!(d.main().unwrap(), DockNode::Split { first, .. } if **first == DockNode::Group { panels: vec![1, 2], active: 1 }));
        assert!(d.set_ratio(&[], 2.0));
        assert!(matches!(d.main().unwrap(), DockNode::Split { ratio, .. } if *ratio == 0.9));
        assert!(!d.set_ratio(&[false], 0.5));
    }
}
