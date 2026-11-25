// Copyright (c) Vitaly Golik (mjojo)

use hey_core::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    EaseOut,
    EaseInOut,
    /// CSS-совместимая кубическая кривая Безье `cubic-bezier(x1,y1,x2,y2)`.
    Bezier(f32, f32, f32, f32),
}

impl Easing {
    /// Тема использует `cubic-bezier(.2,.8,.2,1)`.
    pub const THEME: Easing = Easing::Bezier(0.2, 0.8, 0.2, 1.0);

    pub fn parse(s: &str) -> Option<Easing> {
        let s = s.trim();
        match s {
            "linear" => return Some(Easing::Linear),
            "ease-out" => return Some(Easing::EaseOut),
            "ease-in-out" => return Some(Easing::EaseInOut),
            _ => {}
        }
        let inner = s.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
        let v: Vec<f32> = inner.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
        (v.len() == 4).then(|| Easing::Bezier(v[0], v[1], v[2], v[3]))
    }

    /// Прогресс `t` (0..=1) → значение кривой.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => t * t * (3.0 - 2.0 * t),
            Easing::Bezier(x1, y1, x2, y2) => {
                let b = |a: f32, c: f32, s: f32| {
                    let u = 1.0 - s;
                    3.0 * u * u * s * a + 3.0 * u * s * s * c + s * s * s
                };
                // Бинарный поиск параметра s по x(s) = t.
                let (mut lo, mut hi) = (0.0f32, 1.0f32);
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    if b(x1, x2, mid) < t { lo = mid } else { hi = mid }
                }
                b(y1, y2, (lo + hi) / 2.0)
            }
        }
    }
}

/// Значения, между которыми можно интерполировать.
pub trait Lerp: Copy {
    fn lerp(self, to: Self, t: f32) -> Self;
}
impl Lerp for f32 {
    fn lerp(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t
    }
}
impl Lerp for Color {
    fn lerp(self, to: Color, t: f32) -> Color {
        Color::lerp(self, to, t)
    }
}

/// Анимированное значение: `set` запускает переход от текущего к новому,
/// `tick(dt)` двигает время (кадр-независимо, время передаёт вызывающий).
#[derive(Clone, Copy, Debug)]
pub struct Transition<T: Lerp> {
    from: T,
    to: T,
    elapsed_ms: f32,
    duration_ms: f32,
    easing: Easing,
}

impl<T: Lerp> Transition<T> {
    pub fn new(v: T, duration_ms: f32, easing: Easing) -> Self {
        Self { from: v, to: v, elapsed_ms: duration_ms, duration_ms, easing }
    }
    pub fn value(&self) -> T {
        if self.duration_ms <= 0.0 || self.elapsed_ms >= self.duration_ms {
            return self.to;
        }
        self.from.lerp(self.to, self.easing.apply(self.elapsed_ms / self.duration_ms))
    }
    pub fn target(&self) -> T {
        self.to
    }
    /// Новая цель; старт — из текущего значения (без скачка при прерывании).
    pub fn set(&mut self, to: T) {
        self.from = self.value();
        self.to = to;
        self.elapsed_ms = 0.0;
    }
    /// Мгновенно, без анимации.
    pub fn jump(&mut self, v: T) {
        self.from = v;
        self.to = v;
        self.elapsed_ms = self.duration_ms;
    }
    pub fn tick(&mut self, dt_ms: f32) {
        self.elapsed_ms = (self.elapsed_ms + dt_ms).min(self.duration_ms);
    }
    pub fn is_running(&self) -> bool {
        self.elapsed_ms < self.duration_ms
    }
}
