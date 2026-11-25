use hey_core::{Insets, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Дети слева направо.
    Row,
    /// Дети сверху вниз.
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    Center,
    End,
    /// Растянуть по поперечной оси (если размер `Auto`).
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
    /// Лист: размер из `LayoutTree::set_measure` или явный.
    Leaf,
    Flex(Axis),
    /// Дети накладываются друг на друга в области контента.
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
    /// Промежуток между детьми по главной оси.
    pub gap: f32,
    pub align_items: Align,
    pub align_self: Option<Align>,
    pub justify: Justify,
    pub grow: f32,
    pub shrink: f32,
    pub hidden: bool,
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
    /// Пустой лист, поглощающий свободное место.
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
