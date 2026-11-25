# План разработки: собственный декларативный UI-фреймворк на Rust (CPU + GPU)

Рабочее название: **Hey UI** (крейт-прототип в идее называется `custom_ui_engine`).
Источник: `Thinks/idea.md` (диалог-исследование + рабочий монолитный прототип ~7300 строк).
Автор концепции: Vitaly Golik (mjojo / GLK-Dev).

---

## 1. Суть идеи

Собственный UI-фреймворк для desktop-приложений на Rust с собственным дизайном (окна, рамки, табы, кнопки, поля ввода, списки), где:

1. **Семантика и вид разделены.** Дерево описывает *что* на экране и в каком состоянии (`WidgetState`), а `Theme` + рендерер решают *как* это выглядит.
2. **Однонаправленный поток данных (MVU/Elm):** `Event -> Action -> update(AppState) -> view -> Layout -> DrawCmd -> Backend`.
3. **Единый список команд отрисовки `DrawCmd`**, не зависящий от `winit`/`wgpu`/`tiny-skia`.
4. **Два взаимозаменяемых бэкенда:** `CpuBackend` (tiny-skia/fontdue → `softbuffer`) и `GpuBackend` (`wgpu`: instanced SDF-прямоугольники, атлас глифов, scissor). Переключение на лету.
5. **Единая система координат — физические пиксели** (HiDPI применяется один раз при построении UI).
6. **Арена-дерево (`NodeId` + generation)** вместо `Rc<RefCell>`, чтобы не воевать с borrow checker.

### Что уже есть в прототипе (из idea.md)
| Область | Состояние |
|---|---|
| Arena + `Action` + `AppState`, консольный рендер | есть (этап 0) |
| `DrawCmd` (Rect/ClipPush/ClipPop/Text), `Theme` | есть |
| CPU-бэкенд (скругления, рамки, clip, fontdue) | есть |
| GPU-бэкенд (WGSL SDF, атлас 1024², scissor, тень) | есть |
| HiDPI `scale_factor`, вертикальный скролл + скроллбар | есть |
| Текстовое поле: каретка, выделение мышью/Shift, буфер обмена, dbl/triple click, DnD текста, автоскролл, Esc-отмена | есть |
| Undo/Redo с коалесингом, Ctrl+Backspace/Delete, Ctrl+←/→ | есть |
| Фокус: Tab/Shift+Tab, Enter/Space/F2, фокус-индикатор, навигация по табам (←/→/Home/End) | есть |

### Главная проблема прототипа
Всё в одном `main.rs`: `AppState` знает про конкретное приложение (todo-список), `Action` — монолитный enum, виджеты захардкожены в `build_ui`, нет layout-движка (позиции считаются вручную), нет тестов. **Основная работа плана — превратить прототип в переиспользуемую библиотеку.**

---

## 2. Цели и не-цели

**Цели v1.0**
- Библиотека + демо-приложения для Windows (основная платформа), Linux, macOS.
- Набор виджетов: Label, Button, Checkbox, Radio, Toggle, Slider, TextInput, TextArea, List/ScrollView, Tabs, Panel/WindowFrame, Dropdown, Tooltip, Modal/Popup, Menu.
- Layout (flex-подобный), темы-токены, анимации, клавиатурная доступность, IME, копи-паст, DnD.
- Golden-тесты для визуального паритета CPU ↔ GPU.

**Не-цели**
- Не делаем CSS-совместимость и HTML.
- Не пишем свой compute-растеризатор Безье уровня Vello (путь Vello — опциональный будущий бэкенд).
- Не делаем веб/мобильные таргеты в v1 (оставляем архитектурный задел под wasm).

---

## 3. Архитектура (целевая)

### 3.1 Cargo workspace
```
hey-ui/
├─ crates/
│  ├─ hey-core/        # Arena, NodeId(gen), Key, Rect/Size/Color, события, Action-шина, без зависимостей от ОС/GPU
│  ├─ hey-layout/      # Layout (свой простой flex → затем taffy), hit-test
│  ├─ hey-text/        # шрифты, shaping, перенос строк, метрики, TextEditState (каретка/выделение/undo)
│  ├─ hey-style/       # Theme, токены, StyleId, WidgetState, анимируемые значения
│  ├─ hey-widgets/     # Button, Tabs, TextInput, List ... (генерируют DrawCmd)
│  ├─ hey-draw/        # DrawCmd, DisplayList, trait UiRenderer, clip-stack, общая геометрия
│  ├─ hey-render-cpu/  # tiny-skia/свой растеризатор + fontdue
│  ├─ hey-render-gpu/  # wgpu, WGSL, атлас, батчинг
│  ├─ hey-shell/       # winit-интеграция: окно, цикл, DPI, курсоры, clipboard, IME
│  └─ hey-test/        # headless-раннер, golden-скриншоты, симуляция ввода
├─ examples/           # counter, todo, gallery, text_editor
├─ assets/             # шрифты, иконки
└─ Docs/
```
Правило зависимостей: `core` ← `layout/style/text/draw` ← `widgets` ← `shell`. `hey-core`, `hey-layout`, `hey-draw`, `hey-widgets` **не зависят** от `winit`, `wgpu`, `tiny-skia`.

### 3.2 Конвейер кадра
```
OS event (winit) → Input normalizer → HitTest/Focus → Action/Msg
   → update(&mut Model) → view(&Model) -> Element tree
   → Reconcile (diff по Key, перенос локального состояния виджетов)
   → Layout (Rect для каждого узла, физические пиксели)
   → Paint (Theme × WidgetState → DisplayList/DrawCmd)
   → Backend.render(DisplayList, Viewport)
```
Перерисовка **только** по: изменению состояния, ресайзу, активной анимации, таймеру курсора (`ControlFlow::WaitUntil`).

### 3.3 Ключевые решения (ADR-черновик)
| # | Решение | Обоснование |
|---|---|---|
| ADR-1 | MVU + арена; сигналы — опциональный слой позже | Дружелюбно к borrow checker, проще отлаживать |
| ADR-2 | `NodeId { index, generation }` | Безопасное удаление поддеревьев |
| ADR-3 | Координаты — физические пиксели в `DrawCmd` | Нет путаницы hit-test/рендер |
| ADR-4 | Приложение определяет свой `Msg`; фреймворк — только `UiEvent` + `Callback<Msg>` | Убираем захардкоженный todo-`Action` |
| ADR-5 | Текстовый layout (позиции глифов) делается один раз в `hey-text`, оба бэкенда потребляют готовые глифы | Паритет CPU/GPU |
| ADR-6 | Сначала CPU-бэкенд как эталон, GPU сверяется с ним golden-тестами | Отладка и регрессии |
| ADR-7 | Layout: свой минимальный flex → миграция на `taffy` при росте требований | Не блокировать ранние этапы |
| ADR-8 | Текст: `fontdue` для старта → `cosmic-text`/`parley` (shaping, bidi, fallback, IME) | Кириллица ок, но сложные скрипты требуют shaping |

### 3.4 Дизайн HeOS и влияние на архитектуру
Источник истины — `design/tokens/*.json` (`tokens.json`, `themes.json`, `charts.json`); инструкция — `design/themes/README.md`.
- **Срез вместо скругления.** `DrawCmd::Rect` получает `chamfer` (срез 45° сверху-слева и снизу-справа; размеры small/medium/large) вместо `radii`. Нужны поддержка в SDF-шейдере (GPU) и в CPU-растеризаторе (многоугольник из 6 точек, как `ChamferedRect` в `heos_design.h`).
- **Три темы:** `industrial-dark` (flat), `night-ops` (outline, свечение ≤ 8 px), `zeta-remastered` (bevel). `Theme` содержит `ControlStyle {Flat, Outline, Bevel}`, `WindowEffect`, `chamfer`, `motion_ms`, `bevel`, ANSI-палитру. Размеры и отступы общие, отличается только отрисовка виджета.
- **Тест токенов:** golden-тест сверяет `Theme` всех тем с `themes.json` (по образцу `tests/step1.rs` из дизайна).
- **Шрифты:** Onest (UI, есть кириллица), JetBrains Mono (моно), Space Grotesk (только латиница, бренд) — `design/fonts`, лицензия OFL.
- **Анимации:** тайминги и кривые из темы; режим «уменьшить движение» отключает блик/мигание/развёртку; перерисовка только при активной анимации.
- **Иконки:** SVG из `design/icons` (рендер через `resvg`/`tiny-skia` в атлас), курсоры — `design/cursors` (горячие точки в README).
- **Слой стыковки окон («нейронная связь»)** — отдельный оверлей поверх окон; задача для фазы 9+.

OPEN QUESTION: размеры срезов расходятся — `tokens.json` и `heos_design.h`: 5/9/18; `themes.json` (industrial-dark, night-ops): 8/16/18, zeta-remastered: 4/8/14, а `themes/README.md` говорит 5/9/18 и 0/0/0. Какие значения считать эталонными (пока берём `tokens.json`: 5/9/18)?
OPEN QUESTION: фон рабочего стола Industrial Dark: `#0B0C10` (tokens.json) или `#14161B` (отчёт по темам).

---

## 4. Дорожная карта по фазам

Оценки — для одного разработчика, ориентировочно (чел.-недели, Н). Итого ≈ 30–38 Н до v1.0.

### Фаза 0. Подготовка и фиксация базы (1 Н)
- [x] Создать workspace (корневой `Cargo.toml`). - [ ] CI (GitHub Actions: `cargo fmt --check`, `clippy -D warnings`, `cargo test`, Windows+Linux).
- [x] Перенести прототип из `idea.md` в `examples/prototype_monolith` (компилируется; запуск не проверен; правки: `CachedGlyph: Clone+Copy`, лишний `&` у bind group).
- [ ] Зафиксировать версии зависимостей (`winit 0.30`, `wgpu 24`, `softbuffer 0.4`, `fontdue 0.9`, `bytemuck`, `pollster`).
- [ ] Написать `Docs/ARCHITECTURE.md` и `Docs/ADR/*` по таблице 3.3.
- [ ] Шрифт по умолчанию: Onest (OFL, кириллица) из `design/fonts` — решено дизайном.

**Критерий готовности:** монолит собирается и запускается, CI зелёный.

### Фаза 1. Ядро: типы, арена, события (2 Н) — `hey-core`
- [ ] `Rect`, `Size`, `Point`, `Color`, `Insets`, `Edges`.
- [ ] `Arena<Node>` с `NodeId{index,gen}`, вставка/удаление поддеревьев, free-list, итераторы обхода.
- [ ] `Key` для узлов; `WidgetStateStore` (локальное состояние по ключу: скролл, каретка).
- [ ] Нормализованные события `UiEvent` (Pointer{Move,Down,Up,Wheel}, Key, Text, Focus, Resize, ScaleFactor).
- [ ] Обобщить `Action` → пользовательский `Msg`; `Callback<Msg>`/`MsgSink`.
- [ ] Unit-тесты арены (удаление, переиспользование слотов, устаревшие ID).

### Фаза 2. Draw-слой (`hey-draw`) и CPU-бэкенд эталон (3 Н)
- [ ] `DrawCmd`: `Rect{fill,border,chamfer,shadow}` (срез 45° вместо радиусов, см. 3.4), `Line`, `Text`/`GlyphRun`, `Image`, `ClipPush/Pop`, `Opacity/Layer` (позже).
- [ ] `DisplayList` + clip-stack, валидация парности Push/Pop.
- [ ] `trait UiRenderer { render(&mut self, &DisplayList, Viewport); resize; }`.
- [ ] `CpuRenderer`: скругления, рамки, тени, clip, антиалиасинг по правилам из ADR (рамка центрируется на границе), вывод в `softbuffer`.
- [ ] Экспорт кадра в PNG; **headless-рендер без окна** для тестов.
- [ ] Golden-тесты: набор эталонных кадров (rect, radius, border, clip, text).

### Фаза 3. Текст (`hey-text`) (3–4 Н)
- [ ] Загрузка шрифтов, fallback-цепочка, кэш метрик.
- [ ] Layout текста: измерение, перенос строк, выравнивание, многострочность, обрезка с «…».
- [ ] Выделить **чистую модель редактирования** `TextEditState` (текст, каретка, выделение, границы слов/графем, undo/redo с коалесингом, clipboard-trait) из прототипа — без зависимостей от UI. Покрыть property-тестами (Unicode, кириллица, эмодзи, комбинируемые символы).
- [ ] Курсор по графемам (`unicode-segmentation`), границы слов (`unicode-segmentation`/UAX#29) вместо самодельных.
- [ ] Спайк: `cosmic-text`/`parley` как замена fontdue (shaping, bidi) — решение по ADR-8.

### Фаза 4. Layout и hit-test (`hey-layout`) (3 Н)
- [ ] Контейнеры: `Row`, `Column`, `Stack`, `Spacer`; `gap`, `padding`, `margin`, `align`, `justify`, `flex-grow/shrink`, min/max size.
- [ ] Двухпроходный алгоритм (measure → arrange), кэш по dirty-флагам.
- [ ] Hit-test по дереву с учётом clip и скролла (вместо ручных `HitBox`), z-порядок, popup-слои.
- [ ] Скролл-контейнеры (вертикальный/горизонтальный), инерция, скроллбар как виджет.
- [ ] Спайк интеграции `taffy` и сравнение на бенчмарке (ADR-7).
- [ ] Тесты layout (таблица «вход → Rect»).

### Фаза 5. Стили и темы (`hey-style`) (2 Н)
- [ ] `Theme`: цвета, геометрия, типографика (токены из idea.md §2), светлая/тёмная темы, режим «compact».
- [ ] `WidgetState` (normal/hovered/pressed/focused/disabled/selected) как битовые флаги; `StyleId`/варианты (Primary/Secondary/Danger).
- [ ] Загрузка темы из файла (RON/TOML) + hot-reload в dev-режиме.
- [ ] Анимируемые переходы (`Transition<T>`: цвет, прозрачность, позиция), easing; тик-источник для `request_redraw`.

### Фаза 6. Виджеты (`hey-widgets`) (5–6 Н)
Порядок и критерии (каждый — API, состояния, клавиатура, a11y-роль, golden-тест, пример):
1. Label, Button (`normal→hovered→pressed`, активация Enter/Space).
2. Panel / WindowFrame (рамка не «съедает» контент: `border_width` входит в padding).
3. Tabs (слияние активного таба с рамкой панели, навигация ←/→/Home/End, фокус-рамка).
4. TextInput (на базе `TextEditState`: выделение, DnD, автоскролл, Esc-отмена, undo, Ctrl-навигация) + TextArea.
5. List/ScrollView с ключами (сохранение скролла/фокуса при вставке/удалении) и виртуализацией для больших списков.
6. Checkbox, Radio, Toggle, Slider, ProgressBar.
7. Dropdown/ComboBox, Menu/ContextMenu, Tooltip, Modal — через отдельный **overlay-слой** (z-порядок, закрытие по Esc/клику вне).
8. Реализовать **focus manager** на уровне фреймворка: порядок фокуса строится из дерева (замена ручного `focus_order`), focus trap для модалок.

### Фаза 7. Reconcile и API приложения (`hey-shell`, фасад `hey-ui`) (3 Н)
- [ ] Диффинг дерева по `Key`: сохранение локального состояния виджетов между пересборками.
- [ ] Публичный API: `App::new(model).update(fn).view(fn).theme(...).run()`; `Command`/`Task` для фоновых задач (каналы → `Msg`), состояния idle/loading/success/error.
- [ ] Интеграция `winit`: `ApplicationHandler`, ControlFlow (Wait/WaitUntil), курсоры, мульти-окна (позже), Drag-and-drop файлов, системный clipboard (`arboard`), диалоги файлов (`rfd`).
- [ ] Примеры: `counter`, `todo` (повторяет прототип на новом API), `gallery`.

### Фаза 8. GPU-бэкенд (`hey-render-gpu`) (4–5 Н)
- [ ] Рефакторинг прототипа: `Surface` lost/outdated/resize, `present_mode`, выбор адаптера, fallback на CPU при ошибке.
- [ ] Instanced SDF-пайплайн (rect/rounded/border/shadow), `bytemuck` + проверка alignment, единый буфер инстансов с ring-буферами.
- [ ] Атлас глифов: LRU-вытеснение, несколько страниц, подпиксельное позиционирование, SDF/MSDF-вариант для масштабирования (опционально).
- [ ] Батчинг по scissor, минимизация смен пайплайнов/текстур.
- [ ] Изображения/иконки (атлас иконок, растеризация SVG через `resvg` в текстуру при смене масштаба).
- [ ] Golden-сравнение GPU↔CPU с допуском (порог по среднему/максимальному отклонению; цель ≤ 1 px по краям).
- [ ] Рантайм-переключение CPU↔GPU (флаг запуска + горячая клавиша в dev-режиме).

### Фаза 9. Доступность, i18n, платформа (3–4 Н)
- [ ] **IME** (`WindowEvent::Ime`): preedit/commit, позиция окна кандидатов.
- [ ] a11y через `accesskit` (роли/имена/состояния узлов, навигация экранным диктором).
- [ ] RTL/bidi, локализация (`fluent`), форматирование чисел/дат.
- [ ] Системные темы (dark/light auto), High-contrast, `prefers-reduced-motion`.
- [ ] Платформенные нюансы: Windows (DPI awareness, DWM-рамка/кастомный titlebar), macOS (Cmd вместо Ctrl, меню), Linux (Wayland/X11 clipboard, DnD).
- [ ] Стандартные шорткаты по платформе (Ctrl↔Cmd).

### Фаза 10. Качество, производительность, документация (3 Н)
- [ ] Бенчмарки (`criterion`): layout 10k узлов, paint, CPU-растеризация 4K, GPU-кадр; профилирование (`tracing` + Tracy/puffin).
- [ ] Цели: ввод→кадр ≤ 16 мс при 1080p/60 FPS на CPU-бэкенде для типового UI; холостой CPU ≈ 0%; холодный старт < 300 мс.
- [ ] Fuzz/property-тесты текстового редактора и арены (`proptest`, `cargo-fuzz`).
- [ ] Симуляция ввода в `hey-test` (сценарии «клик → ввод → undo → скриншот»).
- [ ] `cargo doc`, гайд «Начало работы», Cookbook, API-ревью, semver-политика, CHANGELOG.
- [ ] Лицензия, публикация на crates.io (`0.1` → `0.x` → `1.0`).

### Фаза 11 (после v1.0, опционально)
- Vello/`vello_cpu` как третий бэкенд для сложной векторной графики.
- Реактивные сигналы поверх MVU, proc-macro `ui!{}`.
- Виджет-инспектор (devtools): дерево, Rect'ы, стили, перфоманс-оверлей.
- Web (wasm + WebGPU), мобильные платформы.
- Docking/multi-window, графики/node-editor.

---

## 5. Рефакторинг прототипа: карта миграции

| Кусок монолита | Куда переезжает |
|---|---|
| `Arena`, `NodeId`, `Node` | `hey-core` (+generation) |
| `DrawCmd`, clip-stack | `hey-draw` |
| `CpuBackend` | `hey-render-cpu` |
| `GpuBackend`, WGSL, атлас | `hey-render-gpu` |
| `Theme` | `hey-style` |
| `TextInputState`, selection, word boundaries, `EditHistory`, coalescing | `hey-text::TextEditState` (чистая логика, без UI) |
| `Action::*` для текста (KeyHome, DeleteWord…) | внутренние команды `TextEditCommand` + маппинг клавиш в `hey-widgets` |
| `FocusTarget`, `focus_order`, `CycleTab`, `FocusTabEdge` | `hey-widgets::focus` (менеджер фокуса, строится из дерева) |
| `HitBox` | заменяется hit-test по дереву в `hey-layout` |
| `todo_scroll_y`, drag/autoscroll поля | `WidgetStateStore` по `Key` |
| `ApplicationHandler`, `about_to_wait`, мигание каретки | `hey-shell` |
| todo-логика | `examples/todo` |

Порядок: сначала выносим чистые, легко тестируемые части (text edit, arena, draw), затем бэкенды, затем layout/виджеты. После каждого шага — прототип/пример должен продолжать запускаться.

---

## 6. Тестовая стратегия

| Уровень | Что проверяем | Инструмент |
|---|---|---|
| Unit | арена, геометрия, TextEditState, layout-таблицы | `cargo test` |
| Property/Fuzz | Unicode-редактирование, undo/redo-инварианты (`undo∘redo = id`), арена | `proptest`, `cargo-fuzz` |
| Golden | пиксельные эталоны CPU; GPU↔CPU с допуском | `hey-test`, `image` |
| Интеграция | сценарии ввода на headless-раннере | `hey-test` |
| Perf | бенчмарки кадров/layout | `criterion` |
| Ручные | HiDPI 100/125/150/200%, 2 монитора, dark/light, кириллица, IME | чек-лист в `Docs/QA.md` |

Золотое правило: изменения рендера без обновления эталонов не мержатся; изменения эталонов требуют ревью.

---

## 7. Риски и меры

| Риск | Вероятность | Мера |
|---|---|---|
| Расхождение CPU/GPU в антиалиасинге | высокая | Единые правила в ADR, golden-тесты с допуском, CPU как эталон |
| Текст: shaping/bidi/IME сложнее ожиданий | высокая | Ранний спайк `cosmic-text`, текстовый layout изолирован в `hey-text` |
| Дрейф API `wgpu`/`winit` | средняя | Пин версий, обновление раз в квартал отдельной задачей |
| Переусложнение (scope creep) | высокая | Вертикальные срезы, обязательный демо-пример на каждую фазу, не-цели из §2 |
| Borrow-checker в коллбэках | средняя | `Msg`-очередь вместо замыканий с захватом `&mut`, арена с ID |
| Surface lost / ресайз / сворачивание на GPU | средняя | Централизованный recreate-путь, тесты ресайза, fallback на CPU |
| Производительность при больших списках | средняя | Виртуализация списка, dirty-tracking layout, батчинг |
| Один разработчик — bus factor | средняя | ADR, документация, CI с первого дня |

---

## 8. Вехи и критерии приёмки

| Веха | Содержание | Критерий |
|---|---|---|
| **M0** (конец Ф0) | workspace, CI, монолит собран | CI зелёный |
| **M1** (Ф1–2) | core + draw + CPU headless | golden-тесты проходят, PNG-рендер примера |
| **M2** (Ф3–5) | текст, layout, темы | экран с панелью/табами/кнопками собран без ручных координат |
| **M3** (Ф6–7) | виджеты + публичный API | `examples/todo` полностью на новом API, паритет с прототипом |
| **M4** (Ф8) | GPU-бэкенд | GPU↔CPU golden в допуске, переключение на лету |
| **M5** (Ф9) | IME, a11y, платформы | сквозной QA-чек-лист на Windows/Linux/macOS |
| **M6** (Ф10) | релиз 0.1 → 1.0 | бенчмарки в целях, документация, crates.io |

---

## 9. Ближайшие 2 недели (конкретные шаги)

1. День 1–2: workspace + CI; прототип в `examples/prototype_monolith`, проверить `cargo run`.
2. День 3–4: `hey-core` (геометрия, арена с generation, тесты).
3. День 5–7: `hey-draw` (`DrawCmd`, `DisplayList`, `UiRenderer`) + вынос `CpuBackend`; headless PNG.
4. День 8–10: golden-тесты; вынос `TextEditState` из прототипа с первыми unit-тестами (selection, word boundaries, undo/coalescing).
5. День 11–14: минимальный flex-layout (Row/Column/padding/gap) и первый экран без ручных координат; ретро по ADR.

---

## 10. Definition of Done (для любой задачи)
- Код соответствует `cargo fmt` и `clippy -D warnings`; без `unwrap()/expect()` в библиотечных крейтах (допустимо в тестах).
- Есть тесты (unit и, где применимо, golden).
- Публичный API задокументирован (`///` + пример).
- Поведение проверено на CPU и GPU бэкендах, при 100% и ≥150% DPI.
- Нет «тихих» ошибок: любое асинхронное действие имеет состояния idle/loading/success/error.
- Заявления «собирается/проходит тесты» делаются только после реального запуска `cargo check` / `cargo test`.
