// Copyright (c) Vitaly Golik (mjojo)

/// Битовые флаги состояния виджета; пустой набор — `NORMAL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WidgetState(u8);

impl WidgetState {
    pub const NORMAL: Self = Self(0);
    pub const HOVERED: Self = Self(1);
    pub const PRESSED: Self = Self(2);
    pub const FOCUSED: Self = Self(4);
    pub const DISABLED: Self = Self(8);
    pub const SELECTED: Self = Self(16);

    pub const fn contains(self, o: Self) -> bool {
        self.0 & o.0 == o.0
    }
    pub const fn with(self, o: Self) -> Self {
        Self(self.0 | o.0)
    }
    pub const fn without(self, o: Self) -> Self {
        Self(self.0 & !o.0)
    }
    pub fn set(&mut self, o: Self, on: bool) {
        *self = if on { self.with(o) } else { self.without(o) };
    }
    pub const fn is_normal(self) -> bool {
        self.0 == 0
    }
    pub const fn hovered(self) -> bool { self.contains(Self::HOVERED) }
    pub const fn pressed(self) -> bool { self.contains(Self::PRESSED) }
    pub const fn focused(self) -> bool { self.contains(Self::FOCUSED) }
    pub const fn disabled(self) -> bool { self.contains(Self::DISABLED) }
    pub const fn selected(self) -> bool { self.contains(Self::SELECTED) }
}

impl std::ops::BitOr for WidgetState {
    type Output = Self;
    fn bitor(self, o: Self) -> Self {
        self.with(o)
    }
}

/// Вариант оформления кнопок и подобных элементов.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Variant {
    #[default]
    Secondary,
    Primary,
    Danger,
}
