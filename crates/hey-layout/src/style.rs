use hey_core::{Insets, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Children left to right.
    Row,
    /// Children top to bottom.
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    Center,
    End,
    /// Stretch along the cross axis (if the size is `Auto`).
    #[default]
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Dim {
    #[default]
    Auto,
    Px(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Leaf: size from `LayoutTree::set_measure` or explicit.
    Leaf,
    Flex(Axis),
    /// Children are stacked on top of each other in the content area.
    Stack,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub kind: Kind,
    pub width: Dim,
    pub height: Dim,
    pub min: Size,
    pub max: Size,
    pub padding: Insets,
    pub margin: Insets,
    /// Gap between children along the main axis.
    pub gap: f32,
    pub align_items: Align,
    pub align_self: Option<Align>,
    pub justify: Justify,
    pub grow: f32,
    pub shrink: f32,
    pub hidden: bool,
    /// Clip children to the node rectangle (for hit-test and rendering).
    pub clip: bool,
    pub z: i32,
    /// Takes part in hit-test (otherwise the event passes through).
    pub hit: bool,
    /// Popup layer: above everything, not clipped by ancestors.
    pub popup: bool,
    pub scroll_x: bool,
    pub scroll_y: bool,
}

impl Style {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            width: Dim::Auto,
            height: Dim::Auto,
            min: Size::new(0.0, 0.0),
            max: Size::new(f32::INFINITY, f32::INFINITY),
            padding: Insets::default(),
            margin: Insets::default(),
            gap: 0.0,
            align_items: Align::Stretch,
            align_self: None,
            justify: Justify::Start,
            grow: 0.0,
            shrink: 1.0,
            hidden: false,
            clip: false,
            z: 0,
            hit: true,
            popup: false,
            scroll_x: false,
            scroll_y: false,
        }
    }
    pub fn row() -> Self {
        Self::new(Kind::Flex(Axis::Row))
    }
    pub fn column() -> Self {
        Self::new(Kind::Flex(Axis::Column))
    }
    pub fn stack() -> Self {
        Self::new(Kind::Stack)
    }
    pub fn leaf() -> Self {
        Self::new(Kind::Leaf)
    }
    /// Empty leaf that absorbs the free space.
    pub fn spacer() -> Self {
        Self::leaf().grow(1.0)
    }
    pub fn width(mut self, px: f32) -> Self {
        self.width = Dim::Px(px);
        self
    }
    pub fn height(mut self, px: f32) -> Self {
        self.height = Dim::Px(px);
        self
    }
    pub fn size(self, w: f32, h: f32) -> Self {
        self.width(w).height(h)
    }
    pub fn min_size(mut self, w: f32, h: f32) -> Self {
        self.min = Size::new(w, h);
        self
    }
    pub fn max_size(mut self, w: f32, h: f32) -> Self {
        self.max = Size::new(w, h);
        self
    }
    pub fn padding(mut self, i: Insets) -> Self {
        self.padding = i;
        self
    }
    pub fn margin(mut self, i: Insets) -> Self {
        self.margin = i;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn align_items(mut self, a: Align) -> Self {
        self.align_items = a;
        self
    }
    pub fn align_self(mut self, a: Align) -> Self {
        self.align_self = Some(a);
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn grow(mut self, g: f32) -> Self {
        self.grow = g;
        self
    }
    pub fn shrink(mut self, s: f32) -> Self {
        self.shrink = s;
        self
    }
}

impl Style {
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

impl Style {
    pub fn clip(mut self) -> Self {
        self.clip = true;
        self
    }
    pub fn z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }
    pub fn pass_through(mut self) -> Self {
        self.hit = false;
        self
    }
    pub fn popup(mut self) -> Self {
        self.popup = true;
        self
    }
    /// Scroll on the given axes (enables clip).
    pub fn scroll(mut self, x: bool, y: bool) -> Self {
        self.scroll_x = x;
        self.scroll_y = y;
        self.clip = true;
        self
    }
}
