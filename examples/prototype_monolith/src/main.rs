// Copyright (c) Vitaly Golik (mjojo)

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use fontdue::{Font, FontSettings};
use softbuffer::{Context as SoftContext, Surface as SoftSurface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

// ============================================================================
// 1. ТЕМА И ВИЗУАЛЬНЫЕ ТОКЕНЫ (КАСТОМНЫЙ ДИЗАЙН)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Color {
    const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }
    }

    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    #[allow(dead_code)]
    fn to_u32_argb(self) -> u32 {
        let a = (self.a.clamp(0.0, 1.0) * 255.0).round() as u32;
        let r = (self.r.clamp(0.0, 1.0) * 255.0).round() as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0).round() as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0).round() as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }

    fn with_alpha(self, alpha: f32) -> Self {
        Self { a: (self.a * alpha).clamp(0.0, 1.0), ..self }
    }
}

#[derive(Clone, Copy, Debug)]
struct Theme {
    window_bg: Color,
    frame_bg: Color,
    frame_border: Color,
    panel_bg: Color,
    panel_border: Color,
    tab_inactive_bg: Color,
    tab_active_bg: Color,
    tab_hover_bg: Color,
    tab_border: Color,
    accent: Color,
    accent_hover: Color,
    accent_pressed: Color,
    btn_bg: Color,
    btn_hover: Color,
    btn_pressed: Color,
    btn_border: Color,
    danger_bg: Color,
    danger_hover: Color,
    text_primary: Color,
    text_muted: Color,
    text_on_accent: Color,
    shadow: Color,
    border_width: f32,
    frame_radius: f32,
    panel_radius: f32,
    tab_radius: f32,
    btn_radius: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            window_bg: Color::rgb(14, 17, 24),
            frame_bg: Color::rgb(22, 27, 38),
            frame_border: Color::rgb(52, 65, 89),
            panel_bg: Color::rgb(27, 33, 46),
            panel_border: Color::rgb(64, 80, 108),
            tab_inactive_bg: Color::rgb(19, 23, 33),
            tab_active_bg: Color::rgb(27, 33, 46),
            tab_hover_bg: Color::rgb(33, 40, 56),
            tab_border: Color::rgb(64, 80, 108),
            accent: Color::rgb(56, 189, 248),
            accent_hover: Color::rgb(125, 211, 252),
            accent_pressed: Color::rgb(14, 165, 233),
            btn_bg: Color::rgb(36, 44, 61),
            btn_hover: Color::rgb(49, 60, 82),
            btn_pressed: Color::rgb(28, 34, 48),
            btn_border: Color::rgb(78, 97, 130),
            danger_bg: Color::rgb(190, 49, 68),
            danger_hover: Color::rgb(225, 68, 90),
            text_primary: Color::rgb(236, 242, 250),
            text_muted: Color::rgb(148, 163, 184),
            text_on_accent: Color::rgb(10, 14, 22),
            shadow: Color::rgba(0, 0, 0, 120),
            border_width: 1.5,
            frame_radius: 10.0,
            panel_radius: 8.0,
            tab_radius: 7.0,
            btn_radius: 6.0,
        }
    }
}

// ============================================================================
// 2. ЕДИНЫЙ СПИСОК КОМАНД ОТРИСОВКИ (DRAW COMMANDS)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }

    fn intersect(&self, other: Rect) -> Rect {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w).max(x1);
        let y2 = (self.y + self.h).min(other.y + other.h).max(y1);
        Rect { x: x1, y: y1, w: x2 - x1, h: y2 - y1 }
    }
}

#[derive(Clone, Debug)]
enum DrawCmd {
    Box {
        rect: Rect,
        fill: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
        shadow_offset: [f32; 2],
        shadow_blur: f32,
        shadow_color: Color,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: Color,
    },
    ClipPush(Rect),
    ClipPop,
}

// ============================================================================
// 3. СОСТОЯНИЕ ПРИЛОЖЕНИЯ, СОБЫТИЯ И HIT-TARGETS
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabId {
    Overview,
    Todos,
    ThemeInfo,
}

#[derive(Clone, Debug)]
enum Action {
    SelectTab(TabId),
    Increment,
    Decrement,
    AddTodo(String),
    ToggleTodo(usize),
    RemoveTodo(usize),
    ToggleBackend,
}

#[derive(Clone, Debug)]
struct TodoItem {
    id: usize,
    title: String,
    done: bool,
}

#[derive(Clone, Debug)]
struct HitBox {
    id: String,
    rect: Rect,
    action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendKind {
    Cpu,
    Gpu,
}

impl BackendKind {
    fn label(self) -> &'static str {
        match self {
            BackendKind::Cpu => "CPU (softbuffer + fontdue)",
            BackendKind::Gpu => "GPU (wgpu SDF + Atlas)",
        }
    }

    fn toggled(self) -> Self {
        match self {
            BackendKind::Cpu => BackendKind::Gpu,
            BackendKind::Gpu => BackendKind::Cpu,
        }
    }
}

struct AppState {
    active_tab: TabId,
    count: i32,
    next_todo_id: usize,
    todos: Vec<TodoItem>,
    hovered_id: Option<String>,
    pressed_id: Option<String>,
    pointer_pos: (f32, f32),
    backend: BackendKind,
    last_frame_ms: f32,
}

impl AppState {
    fn new(initial_backend: BackendKind) -> Self {
        Self {
            active_tab: TabId::Overview,
            count: 3,
            next_todo_id: 4,
            todos: vec![
                TodoItem { id: 1, title: "Единый DrawCmd для CPU и GPU".into(), done: true },
                TodoItem { id: 2, title: "SDF-шейдер рамок и скруглений на WGSL".into(), done: true },
                TodoItem { id: 3, title: "Кастомные табы и панели состояния".into(), done: false },
            ],
            hovered_id: None,
            pressed_id: None,
            pointer_pos: (0.0, 0.0),
            backend: initial_backend,
            last_frame_ms: 0.0,
        }
    }

    fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::SelectTab(tab) => {
                if self.active_tab != tab {
                    self.active_tab = tab;
                    return true;
                }
                false
            }
            Action::Increment => {
                self.count += 1;
                true
            }
            Action::Decrement => {
                self.count -= 1;
                true
            }
            Action::AddTodo(title) => {
                let t = title.trim();
                if !t.is_empty() {
                    self.todos.push(TodoItem { id: self.next_todo_id, title: t.to_string(), done: false });
                    self.next_todo_id += 1;
                    return true;
                }
                false
            }
            Action::ToggleTodo(id) => {
                if let Some(item) = self.todos.iter_mut().find(|x| x.id == id) {
                    item.done = !item.done;
                    return true;
                }
                false
            }
            Action::RemoveTodo(id) => {
                let len_before = self.todos.len();
                self.todos.retain(|x| x.id != id);
                self.todos.len() != len_before
            }
            Action::ToggleBackend => {
                self.backend = self.backend.toggled();
                true
            }
        }
    }
}

// ============================================================================
// 4. ПОСТРОЕНИЕ КАСТОМНОГО UI (РАМКИ, ТАБЫ, КНОПКИ, ПАНЕЛИ)
// ============================================================================

struct UiContext<'a> {
    theme: &'a Theme,
    state: &'a AppState,
    font: &'a Font,
    cmds: Vec<DrawCmd>,
    hits: Vec<HitBox>,
}

impl<'a> UiContext<'a> {
    fn new(theme: &'a Theme, state: &'a AppState, font: &'a Font) -> Self {
        Self { theme, state, font, cmds: Vec::new(), hits: Vec::new() }
    }

    fn measure_text(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|ch| self.font.metrics(ch, size).advance_width).sum()
    }

    fn draw_box(
        &mut self,
        rect: Rect,
        fill: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
        shadow: Option<([f32; 2], f32, Color)>,
    ) {
        let (shadow_offset, shadow_blur, shadow_color) = match shadow {
            Some((off, blur, col)) => (off, blur, col),
            None => ([0.0, 0.0], 0.0, Color::rgba(0, 0, 0, 0)),
        };
        self.cmds.push(DrawCmd::Box { rect, fill, border_color, border_width, radius, shadow_offset, shadow_blur, shadow_color });
    }

    fn draw_text(&mut self, x: f32, y: f32, text: impl Into<String>, size: f32, color: Color) {
        self.cmds.push(DrawCmd::Text { x, y, text: text.into(), size, color });
    }

    fn button(&mut self, id: &str, rect: Rect, label: &str, action: Action, accent: bool, danger: bool) {
        let is_hovered = self.state.hovered_id.as_deref() == Some(id);
        let is_pressed = self.state.pressed_id.as_deref() == Some(id);

        let (fill, border, text_col) = if danger {
            let f = if is_hovered { self.theme.danger_hover } else { self.theme.danger_bg };
            (f, self.theme.danger_hover, self.theme.text_primary)
        } else if accent {
            let f = if is_pressed {
                self.theme.accent_pressed
            } else if is_hovered {
                self.theme.accent_hover
            } else {
                self.theme.accent
            };
            (f, self.theme.accent_hover, self.theme.text_on_accent)
        } else {
            let f = if is_pressed {
                self.theme.btn_pressed
            } else if is_hovered {
                self.theme.btn_hover
            } else {
                self.theme.btn_bg
            };
            (f, self.theme.btn_border, self.theme.text_primary)
        };

        let offset_y = if is_pressed { 1.0 } else { 0.0 };
        let draw_rect = Rect { y: rect.y + offset_y, ..rect };

        self.draw_box(
            draw_rect,
            fill,
            border,
            self.theme.border_width,
            self.theme.btn_radius,
            if !is_pressed { Some(([0.0, 2.0], 5.0, self.theme.shadow.with_alpha(0.55))) } else { None },
        );

        let text_size = 15.0;
        let text_w = self.measure_text(label, text_size);
        let tx = draw_rect.x + (draw_rect.w - text_w) * 0.5;
        let ty = draw_rect.y + (draw_rect.h - text_size) * 0.5 - 1.0;
        self.draw_text(tx, ty, label, text_size, text_col);

        self.hits.push(HitBox { id: id.to_string(), rect, action });
    }

    fn tab(&mut self, id: &str, rect: Rect, label: &str, active: bool, tab_id: TabId) {
        let is_hovered = self.state.hovered_id.as_deref() == Some(id);

        let fill = if active {
            self.theme.tab_active_bg
        } else if is_hovered {
            self.theme.tab_hover_bg
        } else {
            self.theme.tab_inactive_bg
        };

        let border_col = if active { self.theme.accent } else { self.theme.tab_border };

        // Рисуем корпус таба; если активен — добавляем акцентную полосу сверху
        self.draw_box(
            rect,
            fill,
            border_col,
            if active { self.theme.border_width + 0.5 } else { self.theme.border_width },
            self.theme.tab_radius,
            None,
        );

        if active {
            self.draw_box(
                Rect { x: rect.x + 8.0, y: rect.y, w: (rect.w - 16.0).max(4.0), h: 3.0 },
                self.theme.accent,
                Color::rgba(0, 0, 0, 0),
                0.0,
                1.5,
                None,
            );
        }

        let text_size = 15.0;
        let text_w = self.measure_text(label, text_size);
        let tx = rect.x + (rect.w - text_w) * 0.5;
        let ty = rect.y + (rect.h - text_size) * 0.5;
        let text_col = if active { self.theme.text_primary } else { self.theme.text_muted };
        self.draw_text(tx, ty, label, text_size, text_col);

        self.hits.push(HitBox { id: id.to_string(), rect, action: Action::SelectTab(tab_id) });
    }
}

fn build_ui(width: f32, height: f32, theme: &Theme, state: &AppState, font: &Font) -> (Vec<DrawCmd>, Vec<HitBox>) {
    let mut ui = UiContext::new(theme, state, font);

    // Фон окна
    ui.draw_box(Rect { x: 0.0, y: 0.0, w: width, h: height }, theme.window_bg, Color::rgba(0, 0, 0, 0), 0.0, 0.0, None);

    // Внешняя рамка окна / контейнера
    let margin = 16.0;
    let frame = Rect { x: margin, y: margin, w: (width - margin * 2.0).max(200.0), h: (height - margin * 2.0).max(160.0) };

    ui.draw_box(frame, theme.frame_bg, theme.frame_border, theme.border_width, theme.frame_radius, Some(([0.0, 6.0], 14.0, theme.shadow)));

    // Шапка рамки + переключатель CPU / GPU
    let header_y = frame.y + 14.0;
    ui.draw_text(frame.x + 20.0, header_y + 4.0, "CUSTOM RUST UI ENGINE", 18.0, theme.text_primary);

    let status_text = format!("Режим: {}  |  Кадр: {:.2} ms  |  [Tab] переключить", state.backend.label(), state.last_frame_ms);
    ui.draw_text(frame.x + 20.0, header_y + 28.0, status_text, 13.0, theme.text_muted);

    let toggle_w = 190.0;
    let toggle_h = 34.0;
    ui.button(
        "btn_toggle_backend",
        Rect { x: frame.x + frame.w - toggle_w - 20.0, y: header_y + 4.0, w: toggle_w, h: toggle_h },
        "Переключить CPU / GPU",
        Action::ToggleBackend,
        true,
        false,
    );

    // Панель табов
    let tab_bar_y = frame.y + 68.0;
    let tab_h = 38.0;
    let tab_w = 156.0;
    let tab_gap = 8.0;
    let tabs = [
        (TabId::Overview, "tab_overview", "Обзор и счётчик"),
        (TabId::Todos, "tab_todos", "Список задач"),
        (TabId::ThemeInfo, "tab_theme", "Стиль и рендер"),
    ];

    for (idx, (tid, hid, title)) in tabs.iter().enumerate() {
        let tx = frame.x + 20.0 + idx as f32 * (tab_w + tab_gap);
        ui.tab(hid, Rect { x: tx, y: tab_bar_y, w: tab_w, h: tab_h }, title, state.active_tab == *tid, *tid);
    }

    // Контентная панель с рамкой
    let content_y = tab_bar_y + tab_h + 6.0;
    let content_rect =
        Rect { x: frame.x + 18.0, y: content_y, w: (frame.w - 36.0).max(120.0), h: (frame.y + frame.h - content_y - 18.0).max(80.0) };

    ui.draw_box(content_rect, theme.panel_bg, theme.panel_border, theme.border_width, theme.panel_radius, None);

    ui.cmds.push(DrawCmd::ClipPush(Rect {
        x: content_rect.x + 2.0,
        y: content_rect.y + 2.0,
        w: (content_rect.w - 4.0).max(1.0),
        h: (content_rect.h - 4.0).max(1.0),
    }));

    let inner_x = content_rect.x + 20.0;
    let mut cursor_y = content_rect.y + 20.0;

    match state.active_tab {
        TabId::Overview => {
            ui.draw_text(inner_x, cursor_y, "Интерактивный счётчик", 18.0, theme.text_primary);
            cursor_y += 34.0;

            ui.button("btn_dec", Rect { x: inner_x, y: cursor_y, w: 54.0, h: 40.0 }, "-1", Action::Decrement, false, false);

            ui.draw_text(inner_x + 74.0, cursor_y + 9.0, format!("Значение: {}", state.count), 20.0, theme.accent_hover);

            ui.button("btn_inc", Rect { x: inner_x + 230.0, y: cursor_y, w: 54.0, h: 40.0 }, "+1", Action::Increment, true, false);

            cursor_y += 66.0;
            ui.draw_text(
                inner_x,
                cursor_y,
                "Наведите курсор на кнопки и вкладки — состояния обновляются в реальном времени.",
                14.0,
                theme.text_muted,
            );
            cursor_y += 24.0;
            ui.draw_text(
                inner_x,
                cursor_y,
                "Оба бэкенда читают один и тот же вектор DrawCmd: прямоугольники, рамки, скругления и текст.",
                14.0,
                theme.text_muted,
            );
        }
        TabId::Todos => {
            ui.button(
                "btn_add_todo",
                Rect { x: inner_x, y: cursor_y, w: 190.0, h: 36.0 },
                "+ Добавить задачу",
                Action::AddTodo(format!("Задача #{}", state.next_todo_id)),
                true,
                false,
            );

            cursor_y += 52.0;

            for todo in &state.todos {
                if cursor_y + 42.0 > content_rect.y + content_rect.h - 12.0 {
                    break;
                }

                let row_rect = Rect { x: inner_x, y: cursor_y, w: (content_rect.w - 40.0).max(160.0), h: 38.0 };

                ui.draw_box(row_rect, theme.frame_bg, theme.btn_border, 1.0, 6.0, None);

                let check_label = if todo.done { "[x]" } else { "[ ]" };
                ui.button(
                    &format!("btn_toggle_{}", todo.id),
                    Rect { x: row_rect.x + 8.0, y: row_rect.y + 5.0, w: 44.0, h: 28.0 },
                    check_label,
                    Action::ToggleTodo(todo.id),
                    todo.done,
                    false,
                );

                let title_col = if todo.done { theme.text_muted } else { theme.text_primary };
                ui.draw_text(row_rect.x + 62.0, row_rect.y + 9.0, &todo.title, 15.0, title_col);

                ui.button(
                    &format!("btn_del_{}", todo.id),
                    Rect { x: row_rect.x + row_rect.w - 86.0, y: row_rect.y + 5.0, w: 78.0, h: 28.0 },
                    "Удалить",
                    Action::RemoveTodo(todo.id),
                    false,
                    true,
                );

                cursor_y += 46.0;
            }
        }
        TabId::ThemeInfo => {
            let lines = [
                "Кастомизация внешнего вида через Theme + DrawCmd:",
                "1. Вся геометрия, скругления, рамки и тени описаны в едином DrawCmd::Box.",
                "2. CPU-бэкенд считает SDF/покрытие пикселей и смешивает в ARGB-буфер (softbuffer).",
                "3. GPU-бэкенд (wgpu) рендерит инстансы через SDF + fwidth в WGSL и атлас глифов.",
                "4. Стек ClipPush / ClipPop задаёт области отсечения для панелей и списков.",
            ];
            for line in lines {
                ui.draw_text(inner_x, cursor_y, line, 14.5, theme.text_primary);
                cursor_y += 28.0;
            }
        }
    }

    ui.cmds.push(DrawCmd::ClipPop);

    (ui.cmds, ui.hits)
}

// ============================================================================
// 5. CPU-БЭКЕНД (SDF ДЛЯ СКРУГЛЁННЫХ РАМОК + FONT DUE ДЛЯ ТЕКСТА)
// ============================================================================

fn sdf_round_box(px: f32, py: f32, rect: Rect, radius: f32) -> f32 {
    let half_w = rect.w * 0.5;
    let half_h = rect.h * 0.5;
    let cx = rect.x + half_w;
    let cy = rect.y + half_h;
    let r = radius.min(half_w).min(half_h).max(0.0);

    let qx = (px - cx).abs() - (half_w - r);
    let qy = (py - cy).abs() - (half_h - r);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    let outside = (ox * ox + oy * oy).sqrt();
    let inside = qx.max(qy).min(0.0);
    outside + inside - r
}

fn blend_argb(dst: u32, src: Color, coverage: f32) -> u32 {
    let alpha = (src.a * coverage).clamp(0.0, 1.0);
    if alpha <= 0.0001 {
        return dst;
    }

    let dr = ((dst >> 16) & 0xFF) as f32 / 255.0;
    let dg = ((dst >> 8) & 0xFF) as f32 / 255.0;
    let db = (dst & 0xFF) as f32 / 255.0;

    let out_r = src.r * alpha + dr * (1.0 - alpha);
    let out_g = src.g * alpha + dg * (1.0 - alpha);
    let out_b = src.b * alpha + db * (1.0 - alpha);

    let r = (out_r.clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (out_g.clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (out_b.clamp(0.0, 1.0) * 255.0).round() as u32;
    (0xFF << 24) | (r << 16) | (g << 8) | b
}

struct CpuBackend {
    font: Font,
}

impl CpuBackend {
    fn new(font: Font) -> Self {
        Self { font }
    }

    fn render_to_buffer(&self, cmds: &[DrawCmd], width: usize, height: usize, buf: &mut [u32]) {
        buf.fill(0xFF0E1118);
        let screen_clip = Rect { x: 0.0, y: 0.0, w: width as f32, h: height as f32 };
        let mut clip_stack = vec![screen_clip];

        for cmd in cmds {
            let active_clip = *clip_stack.last().unwrap_or(&screen_clip);
            match cmd {
                DrawCmd::ClipPush(r) => {
                    clip_stack.push(active_clip.intersect(*r));
                }
                DrawCmd::ClipPop => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                }
                DrawCmd::Box { rect, fill, border_color, border_width, radius, shadow_offset, shadow_blur, shadow_color } => {
                    if shadow_color.a > 0.001 && *shadow_blur > 0.0 {
                        let sh_rect = Rect { x: rect.x + shadow_offset[0], y: rect.y + shadow_offset[1], w: rect.w, h: rect.h };
                        self.raster_box_sdf(
                            buf,
                            width,
                            height,
                            active_clip,
                            sh_rect,
                            *shadow_color,
                            Color::rgba(0, 0, 0, 0),
                            0.0,
                            *radius + *shadow_blur * 0.4,
                            *shadow_blur * 0.7,
                        );
                    }
                    self.raster_box_sdf(buf, width, height, active_clip, *rect, *fill, *border_color, *border_width, *radius, 1.0);
                }
                DrawCmd::Text { x, y, text, size, color } => {
                    let mut pen_x = *x;
                    for ch in text.chars() {
                        let (metrics, bitmap) = self.font.rasterize(ch, *size);
                        let gx0 = (pen_x + metrics.xmin as f32).floor() as i32;
                        let gy0 = (*y + (*size * 0.78) - (metrics.ymin as f32 + metrics.height as f32)).floor() as i32;

                        for row in 0..metrics.height {
                            let py = gy0 + row as i32;
                            if py < active_clip.y.floor() as i32
                                || py >= (active_clip.y + active_clip.h).ceil() as i32
                                || py < 0
                                || py >= height as i32
                            {
                                continue;
                            }
                            for col in 0..metrics.width {
                                let px = gx0 + col as i32;
                                if px < active_clip.x.floor() as i32
                                    || px >= (active_clip.x + active_clip.w).ceil() as i32
                                    || px < 0
                                    || px >= width as i32
                                {
                                    continue;
                                }
                                let cov = bitmap[row * metrics.width + col] as f32 / 255.0;
                                if cov > 0.0 {
                                    let idx = py as usize * width + px as usize;
                                    buf[idx] = blend_argb(buf[idx], *color, cov);
                                }
                            }
                        }
                        pen_x += metrics.advance_width;
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn raster_box_sdf(
        &self,
        buf: &mut [u32],
        width: usize,
        height: usize,
        clip: Rect,
        rect: Rect,
        fill: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
        aa_px: f32,
    ) {
        let pad = aa_px + border_width + 2.0;
        let bound = Rect { x: rect.x - pad, y: rect.y - pad, w: rect.w + pad * 2.0, h: rect.h + pad * 2.0 }.intersect(clip);

        let x_start = (bound.x.floor() as i32).max(0) as usize;
        let y_start = (bound.y.floor() as i32).max(0) as usize;
        let x_end = (bound.x + bound.w).ceil().min(width as f32).max(0.0) as usize;
        let y_end = (bound.y + bound.h).ceil().min(height as f32).max(0.0) as usize;

        for py in y_start..y_end {
            let cy = py as f32 + 0.5;
            let row_offset = py * width;
            for px in x_start..x_end {
                let cx = px as f32 + 0.5;
                let d = sdf_round_box(cx, cy, rect, radius);

                // Заливка внутри контура (d < 0)
                let fill_cov = (0.5 - d / aa_px).clamp(0.0, 1.0);
                let mut pixel = buf[row_offset + px];
                if fill.a > 0.001 && fill_cov > 0.0 {
                    pixel = blend_argb(pixel, fill, fill_cov);
                }

                // Рамка шириной border_width вдоль края d = 0
                if border_width > 0.0 && border_color.a > 0.001 {
                    let half_b = border_width * 0.5;
                    let border_dist = (d.abs() - half_b).abs();
                    let border_cov = (1.0 - border_dist / aa_px).clamp(0.0, 1.0);
                    if border_cov > 0.0 {
                        pixel = blend_argb(pixel, border_color, border_cov);
                    }
                }

                buf[row_offset + px] = pixel;
            }
        }
    }
}

// ============================================================================
// 6. GPU-БЭКЕНД (WGPU + SDF-ШЕЙДЕР + ДИНАМИЧЕСКИЙ АТЛАС ГЛИФОВ + SCISSOR)
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScreenUniform {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BoxInstance {
    rect: [f32; 4], // x, y, w, h
    fill: [f32; 4],
    border_color: [f32; 4],
    style: [f32; 4], // border_width, radius, aa_px, _pad
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlyphInstance {
    rect: [f32; 4], // x, y, w, h
    uv: [f32; 4],   // u0, v0, u1, v1
    color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    ch: char,
    size_bits: u32,
}

#[derive(Clone, Copy)]
struct CachedGlyph {
    uv: [f32; 4],
    offset: [f32; 2],
    size: [f32; 2],
    advance: f32,
}

enum GpuBatch {
    Boxes { clip: Rect, start: u32, end: u32 },
    Glyphs { clip: Rect, start: u32, end: u32 },
}

const ATLAS_SIZE: u32 = 1024;

struct GpuBackend {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    font: Font,

    uniform_buffer: wgpu::Buffer,
    box_bind_group: wgpu::BindGroup,
    box_pipeline: wgpu::RenderPipeline,

    glyph_bind_group: wgpu::BindGroup,
    glyph_pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_cursor_x: u32,
    atlas_cursor_y: u32,
    atlas_row_h: u32,
    glyph_cache: HashMap<GlyphKey, CachedGlyph>,

    box_buffer: wgpu::Buffer,
    box_capacity: usize,
    glyph_buffer: wgpu::Buffer,
    glyph_capacity: usize,
}

const SDF_BOX_SHADER: &str = r#"
struct ScreenUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u_screen: ScreenUniform;

struct BoxIn {
    @location(0) rect: vec4<f32>,
    @location(1) fill: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) style: vec4<f32>, // x: border_w, y: radius, z: aa_px
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) fill: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) style: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, instance: BoxIn) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[vid];
    let pad = max(instance.style.x + instance.style.z + 2.0, 2.0);
    let px = instance.rect.x - pad + c.x * (instance.rect.z + pad * 2.0);
    let py = instance.rect.y - pad + c.y * (instance.rect.w + pad * 2.0);

    let ndc_x = (px / u_screen.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / u_screen.screen_size.y) * 2.0;

    let half_size = instance.rect.zw * 0.5;
    let center = instance.rect.xy + half_size;

    var out: VsOut;
    out.pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.local_pos = vec2<f32>(px, py) - center;
    out.half_size = half_size;
    out.fill = instance.fill;
    out.border_color = instance.border_color;
    out.style = instance.style;
    return out;
}

fn sdf_round_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let rad = min(max(r, 0.0), min(b.x, b.y));
    let q = abs(p) - (b - vec2<f32>(rad, rad));
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - rad;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = sdf_round_box(in.local_pos, in.half_size, in.style.y);
    let aa = max(fwidth(d), 0.75);

    let fill_alpha = clamp(0.5 - d / aa, 0.0, 1.0) * in.fill.a;
    var color = vec4<f32>(in.fill.rgb, fill_alpha);

    let border_w = in.style.x;
    if (border_w > 0.0 && in.border_color.a > 0.001) {
        let half_b = border_w * 0.5;
        let b_dist = abs(abs(d) - half_b);
        let b_alpha = clamp(1.0 - b_dist / aa, 0.0, 1.0) * in.border_color.a;
        let out_a = b_alpha + color.a * (1.0 - b_alpha);
        if (out_a > 0.0001) {
            let out_rgb = (in.border_color.rgb * b_alpha + color.rgb * color.a * (1.0 - b_alpha)) / out_a;
            color = vec4<f32>(out_rgb, out_a);
        }
    }

    if (color.a <= 0.001) {
        discard;
    }
    return color;
}
"#;

const GLYPH_SHADER: &str = r#"
struct ScreenUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u_screen: ScreenUniform;

@group(1) @binding(0)
var t_atlas: texture_2d<f32>;

@group(1) @binding(1)
var s_atlas: sampler;

struct GlyphIn {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, instance: GlyphIn) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[vid];
    let px = instance.rect.x + c.x * instance.rect.z;
    let py = instance.rect.y + c.y * instance.rect.w;

    let ndc_x = (px / u_screen.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / u_screen.screen_size.y) * 2.0;

    let u = mix(instance.uv.x, instance.uv.z, c.x);
    let v = mix(instance.uv.y, instance.uv.w, c.y);

    var out: VsOut;
    out.pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = vec2<f32>(u, v);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cov = textureSample(t_atlas, s_atlas, in.uv).r;
    let alpha = cov * in.color.a;
    if (alpha <= 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, alpha);
}
"#;

impl GpuBackend {
    fn new(window: Arc<Window>, width: u32, height: u32, font: Font) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).map_err(|e| format!("Не удалось создать wgpu Surface: {e}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "Не найден подходящий GPU-адаптер wgpu".to_string())?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("ui_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| format!("Ошибка создания wgpu Device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen_uniform"),
            size: std::mem::size_of::<ScreenUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });

        let box_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("box_bind_group"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
        });

        let sdf_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sdf_box_shader"),
            source: wgpu::ShaderSource::Wgsl(SDF_BOX_SHADER.into()),
        });

        let box_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("box_pipeline_layout"),
            bind_group_layouts: &[&uniform_bgl],
            push_constant_ranges: &[],
        });

        let alpha_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let box_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("box_pipeline"),
            layout: Some(&box_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sdf_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BoxInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 32, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 48, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sdf_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(alpha_blend), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d { width: ATLAS_SIZE, height: ATLAS_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let atlas_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let glyph_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph_bind_group"),
            layout: &atlas_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });

        let glyph_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glyph_shader"),
            source: wgpu::ShaderSource::Wgsl(GLYPH_SHADER.into()),
        });

        let glyph_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glyph_pipeline_layout"),
            bind_group_layouts: &[&uniform_bgl, &atlas_bgl],
            push_constant_ranges: &[],
        });

        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glyph_pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &glyph_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 16, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 32, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &glyph_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(alpha_blend), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let box_capacity = 512;
        let box_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("box_instance_buffer"),
            size: (box_capacity * std::mem::size_of::<BoxInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let glyph_capacity = 4096;
        let glyph_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glyph_instance_buffer"),
            size: (glyph_capacity * std::mem::size_of::<GlyphInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            font,
            uniform_buffer,
            box_bind_group,
            box_pipeline,
            glyph_bind_group,
            glyph_pipeline,
            atlas_texture,
            atlas_cursor_x: 1,
            atlas_cursor_y: 1,
            atlas_row_h: 0,
            glyph_cache: HashMap::new(),
            box_buffer,
            box_capacity,
            glyph_buffer,
            glyph_capacity,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn ensure_glyph(&mut self, ch: char, size: f32) -> Option<CachedGlyph> {
        let key = GlyphKey { ch, size_bits: size.to_bits() };
        if let Some(g) = self.glyph_cache.get(&key) {
            return Some(*g);
        }

        let (metrics, bitmap) = self.font.rasterize(ch, size);
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;

        if gw == 0 || gh == 0 {
            let g = CachedGlyph { uv: [0.0; 4], offset: [0.0; 2], size: [0.0; 2], advance: metrics.advance_width };
            self.glyph_cache.insert(key, g);
            return Some(g);
        }

        if gw + 2 > ATLAS_SIZE || gh + 2 > ATLAS_SIZE {
            return None;
        }

        if self.atlas_cursor_x + gw + 2 > ATLAS_SIZE {
            self.atlas_cursor_x = 1;
            self.atlas_cursor_y += self.atlas_row_h + 2;
            self.atlas_row_h = 0;
        }

        if self.atlas_cursor_y + gh + 2 > ATLAS_SIZE {
            // Простой сброс атласа при переполнении
            self.atlas_cursor_x = 1;
            self.atlas_cursor_y = 1;
            self.atlas_row_h = 0;
            self.glyph_cache.clear();
        }

        let x = self.atlas_cursor_x;
        let y = self.atlas_cursor_y;

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(gw), rows_per_image: Some(gh) },
            wgpu::Extent3d { width: gw, height: gh, depth_or_array_layers: 1 },
        );

        self.atlas_cursor_x += gw + 2;
        self.atlas_row_h = self.atlas_row_h.max(gh);

        let f_size = ATLAS_SIZE as f32;
        let g = CachedGlyph {
            uv: [x as f32 / f_size, y as f32 / f_size, (x + gw) as f32 / f_size, (y + gh) as f32 / f_size],
            offset: [metrics.xmin as f32, size * 0.78 - (metrics.ymin as f32 + gh as f32)],
            size: [gw as f32, gh as f32],
            advance: metrics.advance_width,
        };
        self.glyph_cache.insert(key, g);
        Some(g)
    }

    fn render(&mut self, cmds: &[DrawCmd]) -> Result<(), String> {
        let w = self.config.width.max(1);
        let h = self.config.height.max(1);

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&ScreenUniform { screen_size: [w as f32, h as f32], _pad: [0.0, 0.0] }),
        );

        let screen_clip = Rect { x: 0.0, y: 0.0, w: w as f32, h: h as f32 };
        let mut clip_stack = vec![screen_clip];

        let mut boxes: Vec<BoxInstance> = Vec::new();
        let mut glyphs: Vec<GlyphInstance> = Vec::new();
        let mut batches: Vec<GpuBatch> = Vec::new();

        for cmd in cmds {
            let clip = *clip_stack.last().unwrap_or(&screen_clip);
            match cmd {
                DrawCmd::ClipPush(r) => clip_stack.push(clip.intersect(*r)),
                DrawCmd::ClipPop => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                }
                DrawCmd::Box { rect, fill, border_color, border_width, radius, shadow_offset, shadow_blur, shadow_color } => {
                    if shadow_color.a > 0.001 && *shadow_blur > 0.0 {
                        let start = boxes.len() as u32;
                        boxes.push(BoxInstance {
                            rect: [rect.x + shadow_offset[0], rect.y + shadow_offset[1], rect.w, rect.h],
                            fill: shadow_color.with_alpha(0.5).to_array(),
                            border_color: [0.0; 4],
                            style: [0.0, *radius + *shadow_blur * 0.4, *shadow_blur * 0.6, 0.0],
                        });
                        let end = boxes.len() as u32;
                        push_box_batch(&mut batches, clip, start, end);
                    }

                    let start = boxes.len() as u32;
                    boxes.push(BoxInstance {
                        rect: [rect.x, rect.y, rect.w, rect.h],
                        fill: fill.to_array(),
                        border_color: border_color.to_array(),
                        style: [*border_width, *radius, 1.0, 0.0],
                    });
                    let end = boxes.len() as u32;
                    push_box_batch(&mut batches, clip, start, end);
                }
                DrawCmd::Text { x, y, text, size, color } => {
                    let mut pen_x = *x;
                    let start = glyphs.len() as u32;
                    for ch in text.chars() {
                        if let Some(g) = self.ensure_glyph(ch, *size) {
                            if g.size[0] > 0.0 && g.size[1] > 0.0 {
                                glyphs.push(GlyphInstance {
                                    rect: [pen_x + g.offset[0], *y + g.offset[1], g.size[0], g.size[1]],
                                    uv: g.uv,
                                    color: color.to_array(),
                                });
                            }
                            pen_x += g.advance;
                        }
                    }
                    let end = glyphs.len() as u32;
                    if end > start {
                        push_glyph_batch(&mut batches, clip, start, end);
                    }
                }
            }
        }

        if boxes.len() > self.box_capacity {
            self.box_capacity = boxes.len().next_power_of_two();
            self.box_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("box_instance_buffer"),
                size: (self.box_capacity * std::mem::size_of::<BoxInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !boxes.is_empty() {
            self.queue.write_buffer(&self.box_buffer, 0, bytemuck::cast_slice(&boxes));
        }

        if glyphs.len() > self.glyph_capacity {
            self.glyph_capacity = glyphs.len().next_power_of_two();
            self.glyph_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("glyph_instance_buffer"),
                size: (self.glyph_capacity * std::mem::size_of::<GlyphInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !glyphs.is_empty() {
            self.queue.write_buffer(&self.glyph_buffer, 0, bytemuck::cast_slice(&glyphs));
        }

        let frame = self.surface.get_current_texture().map_err(|e| format!("Ошибка получения SurfaceTexture: {e}"))?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ui_frame_encoder") });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.055, g: 0.067, b: 0.094, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            for batch in batches {
                match batch {
                    GpuBatch::Boxes { clip, start, end } => {
                        if let Some((sx, sy, sw, sh)) = to_scissor_rect(clip, w, h) {
                            rpass.set_scissor_rect(sx, sy, sw, sh);
                            rpass.set_pipeline(&self.box_pipeline);
                            rpass.set_bind_group(0, &self.box_bind_group, &[]);
                            rpass.set_vertex_buffer(0, self.box_buffer.slice(..));
                            rpass.draw(0..6, start..end);
                        }
                    }
                    GpuBatch::Glyphs { clip, start, end } => {
                        if let Some((sx, sy, sw, sh)) = to_scissor_rect(clip, w, h) {
                            rpass.set_scissor_rect(sx, sy, sw, sh);
                            rpass.set_pipeline(&self.glyph_pipeline);
                            rpass.set_bind_group(0, self.uniform_buffer_bg_ref(&self.box_bind_group), &[]);
                            rpass.set_bind_group(1, &self.glyph_bind_group, &[]);
                            rpass.set_vertex_buffer(0, self.glyph_buffer.slice(..));
                            rpass.draw(0..6, start..end);
                        }
                    }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    fn uniform_buffer_bg_ref<'a>(&'a self, bg: &'a wgpu::BindGroup) -> &'a wgpu::BindGroup {
        bg
    }
}

fn push_box_batch(batches: &mut Vec<GpuBatch>, clip: Rect, start: u32, end: u32) {
    if let Some(GpuBatch::Boxes { clip: prev_clip, end: prev_end, .. }) = batches.last_mut() {
        if *prev_clip == clip && *prev_end == start {
            *prev_end = end;
            return;
        }
    }
    batches.push(GpuBatch::Boxes { clip, start, end });
}

fn push_glyph_batch(batches: &mut Vec<GpuBatch>, clip: Rect, start: u32, end: u32) {
    if let Some(GpuBatch::Glyphs { clip: prev_clip, end: prev_end, .. }) = batches.last_mut() {
        if *prev_clip == clip && *prev_end == start {
            *prev_end = end;
            return;
        }
    }
    batches.push(GpuBatch::Glyphs { clip, start, end });
}

fn to_scissor_rect(clip: Rect, surface_w: u32, surface_h: u32) -> Option<(u32, u32, u32, u32)> {
    let x = clip.x.floor().max(0.0) as u32;
    let y = clip.y.floor().max(0.0) as u32;
    let right = (clip.x + clip.w).ceil().min(surface_w as f32).max(0.0) as u32;
    let bottom = (clip.y + clip.h).ceil().min(surface_h as f32).max(0.0) as u32;
    if right <= x || bottom <= y {
        return None;
    }
    Some((x, y, right - x, bottom - y))
}

// ============================================================================
// 7. ЗАГРУЗКА ШРИФТА И ГЛАВНЫЙ ЦИКЛ ПРИЛОЖЕНИЯ (WINIT)
// ============================================================================

fn load_system_or_local_font() -> Result<Font, String> {
    let candidates = [
        PathBuf::from("assets/font.ttf"),
        PathBuf::from("C:/Windows/Fonts/segoeui.ttf"),
        PathBuf::from("C:/Windows/Fonts/arial.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
    ];

    for path in candidates {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                return Ok(font);
            }
        }
    }

    Err("Не найден TTF-шрифт. Положите любой .ttf файл (с поддержкой кириллицы) в assets/font.ttf.".to_string())
}

struct App {
    window: Option<Arc<Window>>,
    soft_context: Option<SoftContext<Arc<Window>>>,
    soft_surface: Option<SoftSurface<Arc<Window>, Arc<Window>>>,
    cpu_backend: CpuBackend,
    gpu_backend: Option<GpuBackend>,
    theme: Theme,
    state: AppState,
    queue: VecDeque<Action>,
    last_hits: Vec<HitBox>,
}

impl App {
    fn new(font: Font) -> Self {
        let gpu_font = font.clone();
        Self {
            window: None,
            soft_context: None,
            soft_surface: None,
            cpu_backend: CpuBackend::new(font),
            gpu_backend: None,
            theme: Theme::default(),
            state: AppState::new(BackendKind::Gpu),
            queue: VecDeque::new(),
            last_hits: Vec::new(),
        }
        .with_gpu_font(gpu_font)
    }

    fn with_gpu_font(self, _f: Font) -> Self {
        self
    }

    fn dispatch(&mut self, action: Action) {
        self.queue.push_back(action);
    }

    fn update_hover(&mut self) {
        let (px, py) = self.state.pointer_pos;
        let found = self.last_hits.iter().rev().find(|h| h.rect.contains(px, py)).map(|h| h.id.clone());

        if self.state.hovered_id != found {
            self.state.hovered_id = found;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn render_frame(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let t0 = std::time::Instant::now();
        let (cmds, hits) = build_ui(width as f32, height as f32, &self.theme, &self.state, &self.cpu_backend.font);
        self.last_hits = hits;

        match self.state.backend {
            BackendKind::Gpu => {
                if let Some(gpu) = &mut self.gpu_backend {
                    if let Err(err) = gpu.render(&cmds) {
                        eprintln!("Ошибка GPU-рендера ({err}), переключаюсь на CPU.");
                        self.state.backend = BackendKind::Cpu;
                    }
                } else {
                    self.state.backend = BackendKind::Cpu;
                }
            }
            BackendKind::Cpu => {}
        }

        if self.state.backend == BackendKind::Cpu {
            if let Some(surface) = &mut self.soft_surface {
                let _ = surface.resize(std::num::NonZeroU32::new(width).unwrap(), std::num::NonZeroU32::new(height).unwrap());
                if let Ok(mut buf) = surface.buffer_mut() {
                    self.cpu_backend.render_to_buffer(&cmds, width as usize, height as usize, &mut buf);
                    let _ = buf.present();
                }
            }
        }

        self.state.last_frame_ms = t0.elapsed().as_secs_f32() * 1000.0;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs =
            Window::default_attributes().with_title("Custom Rust UI Engine — CPU + GPU").with_inner_size(LogicalSize::new(920.0, 560.0));

        let window = Arc::new(event_loop.create_window(attrs).expect("Ошибка создания окна"));
        let size = window.inner_size();

        let soft_context = SoftContext::new(window.clone()).ok();
        let soft_surface = soft_context.as_ref().and_then(|ctx| SoftSurface::new(ctx, window.clone()).ok());

        let font_for_gpu = self.cpu_backend.font.clone();
        match GpuBackend::new(window.clone(), size.width, size.height, font_for_gpu) {
            Ok(gpu) => {
                self.gpu_backend = Some(gpu);
                self.state.backend = BackendKind::Gpu;
            }
            Err(e) => {
                eprintln!("{e}. Запуск на CPU-бэкенде.");
                self.state.backend = BackendKind::Cpu;
            }
        }

        self.soft_context = soft_context;
        self.soft_surface = soft_surface;
        self.window = Some(window.clone());
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu_backend {
                    gpu.resize(new_size.width, new_size.height);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.state.pointer_pos = (position.x as f32, position.y as f32);
                self.update_hover();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (px, py) = self.state.pointer_pos;
                match state {
                    ElementState::Pressed => {
                        let hit = self.last_hits.iter().rev().find(|h| h.rect.contains(px, py)).cloned();
                        self.state.pressed_id = hit.map(|h| h.id);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    ElementState::Released => {
                        let released_hit = self.last_hits.iter().rev().find(|h| h.rect.contains(px, py)).cloned();

                        if let (Some(pressed), Some(rel)) = (&self.state.pressed_id, &released_hit) {
                            if pressed == &rel.id {
                                self.dispatch(rel.action.clone());
                            }
                        }
                        self.state.pressed_id = None;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key: PhysicalKey::Code(KeyCode::Tab), state: ElementState::Pressed, repeat: false, .. },
                ..
            } => {
                self.dispatch(Action::ToggleBackend);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let mut changed = false;
                while let Some(action) = self.queue.pop_front() {
                    if self.state.apply(action) {
                        changed = true;
                    }
                }
                let _ = changed;
                self.render_frame();
            }
            _ => {}
        }
    }
}

fn main() {
    let font = load_system_or_local_font().unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    let event_loop = EventLoop::new().expect("Ошибка создания EventLoop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut app = App::new(font);
    event_loop.run_app(&mut app).expect("Ошибка цикла событий");
}
