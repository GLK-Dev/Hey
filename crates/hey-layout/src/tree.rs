use crate::style::{Align, Axis, Dim, Justify, Kind, Style};
use hey_core::{NodeId, Point, Rect, Size, Tree};
use std::cell::RefCell;
use std::collections::HashMap;

type MeasureFn = Box<dyn Fn(Option<f32>) -> Size>;

struct Node {
    style: Style,
    measure: Option<MeasureFn>,
    rect: Rect,
    scroll: Point,
    content: Size,
}

/// Layout tree. The root fills the whole window; other nodes are laid out with the flex algorithm.
pub struct LayoutTree {
    tree: Tree<Node>,
    root: NodeId,
    /// Measurement memo within a single pass: (node, available width) → size.
    cache: RefCell<HashMap<(NodeId, Option<u32>), Size>>,
    dirty: bool,
    viewport: Option<Size>,
}

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    v.max(min).min(max)
}

fn snap(x: f32, y: f32, w: f32, h: f32) -> Rect {
    let (x0, y0) = (x.round(), y.round());
    Rect::new(x0, y0, (x + w).round() - x0, (y + h).round() - y0)
}

fn offset(a: Align, free: f32) -> f32 {
    match a {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => free / 2.0,
        Align::End => free,
    }
}

impl LayoutTree {
    pub fn new(root: Style) -> Self {
        let mut tree = Tree::new();
        let root = tree
            .add(Node { style: root, measure: None, rect: Rect::default(), scroll: Point::default(), content: Size::default() }, None)
            .expect("root");
        Self { tree, root, cache: RefCell::new(HashMap::new()), dirty: true, viewport: None }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn add(&mut self, parent: NodeId, style: Style) -> Option<NodeId> {
        self.dirty = true;
        self.tree
            .add(Node { style, measure: None, rect: Rect::default(), scroll: Point::default(), content: Size::default() }, Some(parent))
    }

    /// Mark the layout stale (e.g. the text measured by a leaf closure changed).
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub fn remove(&mut self, id: NodeId) {
        self.dirty = true;
        self.tree.remove_subtree(id);
    }

    /// Reorders the children of `parent` (a permutation of the current children).
    pub fn reorder_children(&mut self, parent: NodeId, order: &[NodeId]) -> bool {
        self.dirty = true;
        self.tree.reorder_children(parent, order)
    }

    pub fn set_style(&mut self, id: NodeId, style: Style) {
        self.dirty = true;
        if let Some(n) = self.tree.get_mut(id) {
            n.style = style;
        }
    }

    pub fn style(&self, id: NodeId) -> Option<&Style> {
        self.tree.get(id).map(|n| &n.style)
    }

    /// Leaf measure closure: takes the available inner width (if known).
    pub fn set_measure(&mut self, id: NodeId, f: impl Fn(Option<f32>) -> Size + 'static) {
        self.dirty = true;
        if let Some(n) = self.tree.get_mut(id) {
            n.measure = Some(Box::new(f));
        }
    }

    /// Current scroll offset.
    pub fn scroll(&self, id: NodeId) -> Point {
        self.tree.get(id).map(|n| n.scroll).unwrap_or_default()
    }

    /// Maximum scroll offset (after `compute`).
    pub fn max_scroll(&self, id: NodeId) -> Point {
        let Some(n) = self.tree.get(id) else { return Point::default() };
        let v = n.rect.deflate(n.style.padding);
        Point::new((n.content.w - v.w).max(0.0), (n.content.h - v.h).max(0.0))
    }

    /// Set the offset (clamped on the next `compute`).
    pub fn set_scroll(&mut self, id: NodeId, p: Point) {
        self.dirty = true;
        if let Some(n) = self.tree.get_mut(id) {
            n.scroll = p;
        }
    }

    /// Scroll by `d`; returns `true` if the offset changed.
    pub fn scroll_by(&mut self, id: NodeId, dx: f32, dy: f32) -> bool {
        let max = self.max_scroll(id);
        let Some(n) = self.tree.get_mut(id) else { return false };
        let to = Point::new((n.scroll.x + dx).clamp(0.0, max.x), (n.scroll.y + dy).clamp(0.0, max.y));
        let changed = to != n.scroll;
        n.scroll = to;
        self.dirty |= changed;
        changed
    }

    /// Topmost node under the point: popup layers first, then the main tree;
    /// accounts for clip, scroll (via positions), z-order and `pass_through`.
    pub fn hit_test(&self, p: Point) -> Option<NodeId> {
        let mut popups: Vec<(i32, usize, NodeId)> = Vec::new();
        for (i, (id, n)) in self.tree.dfs(self.root).enumerate() {
            if n.style.popup && !n.style.hidden {
                popups.push((n.style.z, i, id));
            }
        }
        popups.sort_by_key(|&(z, i, _)| (std::cmp::Reverse(z), std::cmp::Reverse(i)));
        for (_, _, id) in popups {
            if let Some(h) = self.hit_node(id, p, None, true) {
                return Some(h);
            }
        }
        self.hit_node(self.root, p, None, false)
    }

    fn hit_node(&self, id: NodeId, p: Point, clip: Option<Rect>, is_popup_root: bool) -> Option<NodeId> {
        let n = self.tree.get(id)?;
        if n.style.hidden || (n.style.popup && !is_popup_root) {
            return None;
        }
        if clip.is_some_and(|c| !c.contains(p)) {
            return None;
        }
        let child_clip = if n.style.clip {
            match clip {
                Some(c) => Some(c.intersect(&n.rect)?),
                None => Some(n.rect),
            }
        } else {
            clip
        };
        let mut kids: Vec<(i32, usize, NodeId)> =
            self.tree.children(id).iter().enumerate().map(|(i, &c)| (self.tree.get(c).map_or(0, |k| k.style.z), i, c)).collect();
        kids.sort_by_key(|&(z, i, _)| (std::cmp::Reverse(z), std::cmp::Reverse(i)));
        for (_, _, c) in kids {
            if let Some(h) = self.hit_node(c, p, child_clip, false) {
                return Some(h);
            }
        }
        (n.style.hit && n.rect.contains(p)).then_some(id)
    }

    /// Absolute rectangle of a node (after `compute`).
    pub fn rect(&self, id: NodeId) -> Rect {
        self.tree.get(id).map(|n| n.rect).unwrap_or_default()
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.tree.children(id)
    }

    pub fn compute(&mut self, viewport: Size) {
        if !self.dirty && self.viewport == Some(viewport) {
            return;
        }
        self.cache.borrow_mut().clear();
        let r = Rect::new(0.0, 0.0, viewport.w.round(), viewport.h.round());
        self.arrange(self.root, r);
        self.cache.borrow_mut().clear();
        self.dirty = false;
        self.viewport = Some(viewport);
    }

    fn visible(&self, id: NodeId) -> Vec<NodeId> {
        self.tree.children(id).iter().copied().filter(|&c| !self.tree.get(c).is_some_and(|n| n.style.hidden)).collect()
    }

    /// Border-box size of a node (without outer margins). `avail` is the available border-box width.
    fn measure(&self, id: NodeId, avail: Option<f32>) -> Size {
        let key = (id, avail.map(f32::to_bits));
        if let Some(&s) = self.cache.borrow().get(&key) {
            return s;
        }
        let s = self.measure_uncached(id, avail);
        self.cache.borrow_mut().insert(key, s);
        s
    }

    fn measure_uncached(&self, id: NodeId, avail: Option<f32>) -> Size {
        let Some(n) = self.tree.get(id) else { return Size::default() };
        let st = n.style;
        if st.hidden {
            return Size::default();
        }
        let fixed_w = match st.width {
            Dim::Px(p) => Some(p),
            Dim::Auto => None,
        };
        let w_avail = fixed_w.or(avail).map(|w| clamp(w, st.min.w, st.max.w));
        let inner = w_avail.map(|w| (w - st.padding.horizontal()).max(0.0));
        let content = self.content(id, inner);
        let w = match st.width {
            Dim::Px(p) => p,
            Dim::Auto => content.w + st.padding.horizontal(),
        };
        let h = match st.height {
            Dim::Px(p) => p,
            Dim::Auto => content.h + st.padding.vertical(),
        };
        Size::new(clamp(w, st.min.w, st.max.w), clamp(h, st.min.h, st.max.h))
    }

    /// Content size (without padding); `inner` is the available inner width.
    fn content(&self, id: NodeId, inner: Option<f32>) -> Size {
        let n = self.tree.get(id).unwrap();
        let st = n.style;
        let kids = self.visible(id);
        match st.kind {
            Kind::Leaf => n.measure.as_ref().map_or(Size::default(), |f| f(inner)),
            Kind::Stack => kids.iter().fold(Size::default(), |a, &c| {
                let m = self.tree.get(c).unwrap().style.margin;
                let s = self.measure(c, inner.map(|w| (w - m.horizontal()).max(0.0)));
                Size::new(a.w.max(s.w + m.horizontal()), a.h.max(s.h + m.vertical()))
            }),
            Kind::Flex(axis) => {
                let gaps = st.gap * kids.len().saturating_sub(1) as f32;
                let (mut main, mut cross) = (gaps, 0.0f32);
                for &c in &kids {
                    let m = self.tree.get(c).unwrap().style.margin;
                    let (sm, sc, mm, mc) = match axis {
                        Axis::Row => {
                            let s = self.measure(c, None);
                            (s.w, s.h, m.horizontal(), m.vertical())
                        }
                        Axis::Column => {
                            let s = self.measure(c, inner.map(|w| (w - m.horizontal()).max(0.0)));
                            (s.h, s.w, m.vertical(), m.horizontal())
                        }
                    };
                    main += sm + mm;
                    cross = cross.max(sc + mc);
                }
                if axis == Axis::Row {
                    Size::new(main, cross)
                } else {
                    Size::new(cross, main)
                }
            }
        }
    }

    fn set_rect(&mut self, id: NodeId, r: Rect) {
        if let Some(n) = self.tree.get_mut(id) {
            n.rect = r;
        }
    }

    fn arrange(&mut self, id: NodeId, rect: Rect) {
        self.set_rect(id, rect);
        let st = self.tree.get(id).unwrap().style;
        for c in self.tree.children(id).to_vec() {
            if self.tree.get(c).is_some_and(|n| n.style.hidden) {
                self.set_rect(c, Rect::default());
            }
        }
        let viewport = rect.deflate(st.padding);
        let mut inner = viewport;
        if st.scroll_x || st.scroll_y {
            let c = self.content(id, Some(viewport.w));
            if st.scroll_x {
                inner.w = inner.w.max(c.w);
            }
            if st.scroll_y {
                inner.h = inner.h.max(c.h);
            }
            let max = Point::new((inner.w - viewport.w).max(0.0), (inner.h - viewport.h).max(0.0));
            let n = self.tree.get_mut(id).unwrap();
            n.content = Size::new(inner.w, inner.h);
            n.scroll = Point::new(n.scroll.x.clamp(0.0, max.x), n.scroll.y.clamp(0.0, max.y));
            inner.x -= n.scroll.x.round();
            inner.y -= n.scroll.y.round();
        }
        let kids = self.visible(id);
        match st.kind {
            Kind::Leaf => {}
            Kind::Stack => {
                for c in kids {
                    let cs = self.tree.get(c).unwrap().style;
                    let al = cs.align_self.unwrap_or(st.align_items);
                    let m = cs.margin;
                    let (aw, ah) = ((inner.w - m.horizontal()).max(0.0), (inner.h - m.vertical()).max(0.0));
                    let w = match cs.width {
                        Dim::Px(p) => p,
                        Dim::Auto if al == Align::Stretch => aw,
                        Dim::Auto => self.measure(c, Some(aw)).w,
                    };
                    let w = clamp(w, cs.min.w, cs.max.w);
                    let h = match cs.height {
                        Dim::Px(p) => p,
                        Dim::Auto if al == Align::Stretch => ah,
                        Dim::Auto => self.measure(c, Some(w)).h,
                    };
                    let h = clamp(h, cs.min.h, cs.max.h);
                    let (dx, dy) = (offset(al, aw - w), offset(al, ah - h));
                    self.arrange(c, snap(inner.x + m.left + dx, inner.y + m.top + dy, w, h));
                }
            }
            Kind::Flex(axis) => self.arrange_flex(&st, axis, inner, kids),
        }
    }

    fn arrange_flex(&mut self, st: &Style, axis: Axis, inner: Rect, kids: Vec<NodeId>) {
        struct K {
            id: NodeId,
            st: Style,
            base: f32,
            size: f32,
        }
        let row = axis == Axis::Row;
        let (inner_main, inner_cross) = if row { (inner.w, inner.h) } else { (inner.h, inner.w) };
        let mm = |s: &Style| if row { s.margin.horizontal() } else { s.margin.vertical() };
        let mc = |s: &Style| if row { s.margin.vertical() } else { s.margin.horizontal() };
        let bounds = |s: &Style| if row { (s.min.w, s.max.w) } else { (s.min.h, s.max.h) };

        let mut ks: Vec<K> = kids
            .iter()
            .map(|&id| {
                let cs = self.tree.get(id).unwrap().style;
                let base = if row { self.measure(id, None).w } else { self.measure(id, Some((inner_cross - mc(&cs)).max(0.0))).h };
                K { id, st: cs, base, size: base }
            })
            .collect();
        let n = ks.len();
        let gaps = st.gap * n.saturating_sub(1) as f32;
        let total: f32 = ks.iter().map(|k| k.base + mm(&k.st)).sum::<f32>() + gaps;
        let free = inner_main - total;
        let grow: f32 = ks.iter().map(|k| k.st.grow).sum();
        let shrink: f32 = ks.iter().map(|k| k.st.shrink * k.base).sum();
        for k in &mut ks {
            let (lo, hi) = bounds(&k.st);
            if free > 0.0 && grow > 0.0 {
                k.size = clamp(k.base + free * k.st.grow / grow, lo, hi);
            } else if free < 0.0 && shrink > 0.0 {
                k.size = clamp((k.base + free * k.st.shrink * k.base / shrink).max(0.0), lo, hi);
            }
        }
        let used: f32 = ks.iter().map(|k| k.size + mm(&k.st)).sum::<f32>() + gaps;
        let rem = (inner_main - used).max(0.0);
        let nf = n as f32;
        let (lead, between) = match st.justify {
            Justify::Start => (0.0, st.gap),
            Justify::Center => (rem / 2.0, st.gap),
            Justify::End => (rem, st.gap),
            Justify::SpaceBetween if n > 1 => (0.0, st.gap + rem / (nf - 1.0)),
            Justify::SpaceAround if n > 0 => (rem / nf / 2.0, st.gap + rem / nf),
            Justify::SpaceEvenly if n > 0 => (rem / (nf + 1.0), st.gap + rem / (nf + 1.0)),
            _ => (0.0, st.gap),
        };
        let mut pos = lead;
        for k in ks {
            let cs = k.st;
            let m = cs.margin;
            let (m_start, m_end, c_start) = if row { (m.left, m.right, m.top) } else { (m.top, m.bottom, m.left) };
            let avail = (inner_cross - mc(&cs)).max(0.0);
            let al = cs.align_self.unwrap_or(st.align_items);
            let (clo, chi) = if row { (cs.min.h, cs.max.h) } else { (cs.min.w, cs.max.w) };
            let cross_dim = if row { cs.height } else { cs.width };
            let cross = match cross_dim {
                Dim::Px(p) => p,
                Dim::Auto if al == Align::Stretch => avail,
                Dim::Auto if row => self.measure(k.id, Some(k.size)).h,
                Dim::Auto => self.measure(k.id, Some(avail)).w,
            };
            let cross = clamp(cross, clo, chi);
            let coff = c_start + offset(al, avail - cross);
            pos += m_start;
            let r =
                if row { snap(inner.x + pos, inner.y + coff, k.size, cross) } else { snap(inner.x + coff, inner.y + pos, cross, k.size) };
            self.arrange(k.id, r);
            pos += k.size + m_end + between;
        }
    }
}
