/// Цвет RGBA, 8 бит на канал, не премультиплицированный.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    /// `0xRRGGBB`, непрозрачный. Удобно для токенов: `Color::hex(0xF5A623)`.
    pub const fn hex(v: u32) -> Self {
        Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    /// Разбор `#RRGGBB` или `#RRGGBBAA` (решётка необязательна).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#').unwrap_or(s);
        if !s.is_ascii() || (s.len() != 6 && s.len() != 8) {
            return None;
        }
        let ch = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        let a = if s.len() == 8 { ch(6)? } else { 255 };
        Some(Self::rgba(ch(0)?, ch(2)?, ch(4)?, a))
    }
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
    /// Линейная интерполяция по каналам в sRGB-пространстве, `t` в 0..=1.
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color::rgba(m(self.r, o.r), m(self.g, o.g), m(self.b, o.b), m(self.a, o.a))
    }
    /// Относительная яркость по WCAG.
    pub fn luminance(self) -> f32 {
        let f = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(self.r) + 0.7152 * f(self.g) + 0.0722 * f(self.b)
    }
    /// Коэффициент контраста WCAG (1..=21).
    pub fn contrast(self, o: Color) -> f32 {
        let (a, b) = (self.luminance(), o.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex() {
        assert_eq!(Color::parse("#F5A623"), Some(Color::hex(0xF5A623)));
        assert_eq!(Color::parse("00E5FF80"), Some(Color::rgba(0, 0xE5, 0xFF, 0x80)));
        assert_eq!(Color::parse("#12345"), None);
        assert_eq!(Color::parse("#GG0000"), None);
    }

    /// Сверка с таблицей контраста из design/themes/README.md.
    #[test]
    fn contrast_matches_design_table() {
        let c = Color::hex(0xF2F4F8).contrast(Color::hex(0x181A20));
        assert!((c - 15.8).abs() < 0.15, "{c}");
        let acc = Color::hex(0xF5A623).contrast(Color::hex(0x181A20));
        assert!((acc - 8.6).abs() < 0.15, "{acc}");
    }

    #[test]
    fn lerp_ends() {
        let (a, b) = (Color::hex(0x000000), Color::hex(0xFFFFFF));
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
    }
}
