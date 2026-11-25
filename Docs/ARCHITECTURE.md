# Архитектура Hey

Hey — UI-фреймворк на Rust: декларативный MVU-слой поверх арены виджетов, единый список команд рисования (`DisplayList`) и сменные бэкенды рендера. Подробный план — `DEVELOPMENT_PLAN.md`, обоснования решений — `ADR/`.

## Крейты и зависимости

```
hey-core ← hey-layout, hey-style, hey-text, hey-draw ← hey-widgets ← hey-shell
                                      hey-draw ← hey-render-cpu
```

| Крейт | Назначение |
|-------|------------|
| `hey-core` | Арена `Tree<T>` с `NodeId { index, generation }`, геометрия, `Color`, `UiEvent`, `Callback`/`MsgSink`, `CursorIcon` |
| `hey-layout` | Flex-раскладка (`Style::row/column/stack`, padding, gap, grow) в физических пикселях |
| `hey-style` | `Theme` и токены, `WidgetState`, `Variant`, переходы (`transition`) |
| `hey-text` | Реестр шрифтов (fontdue), раскладка строк, `TextEditState` (каретка, выделение, undo) |
| `hey-draw` | `DrawCmd`, `DisplayList` (с clip-стеком и валидацией), `trait UiRenderer` |
| `hey-widgets` | `Ui<Msg>`, виджеты, `View` + reconcile, фокус, overlay (Menu/Modal/Tooltip/Dropdown), clipboard |
| `hey-render-cpu` | `CpuRenderer`: растеризация `DisplayList` в `Framebuffer`, эталон для golden-тестов |
| `hey-shell` | Окно winit + softbuffer, нормализация ввода, `App`/`Command`/`TaskState`, диалоги (rfd), системный clipboard |

Правило: `core`, `layout`, `draw`, `widgets` не зависят от `winit`, `wgpu`, растеризаторов. Платформа подключается только в `hey-shell`.

## Конвейер кадра

```
winit WindowEvent → InputState::translate → UiEvent
  → Ui::event (hit-test, фокус, overlay) → Msg в MsgSink
  → update(&mut Model, Msg) -> Command
  → view(&Model) -> Vec<View<Msg>> → Ui::reconcile (diff по Key, перенос состояния виджетов)
  → layout (Rect для каждого узла) → paint (Theme × WidgetState) → DisplayList
  → UiRenderer::render → softbuffer
```

Перерисовка запрашивается только после обработанного события, результата фоновой задачи или ресайза; в простое цикл спит (`ControlFlow::Wait`).

## MVU

- Приложение определяет `Model`, `Msg: Send + 'static`, `update`, `view`.
- `App::new(model).update(f).view(f).theme(t).font(path).run()`.
- `Command::{none, msg, perform, batch}`: `perform` выполняет блокирующую работу в потоке и возвращает результат сообщением; `TaskState` описывает idle/loading/success/error.
- Виджеты сообщают о событиях через `Callback<Msg, A>`, которые кладут `Msg` в `MsgSink`; фреймворк не знает типа `Msg`.

## Модель отрисовки

- Все координаты — физические пиксели (ADR-3); масштаб задаётся через `Theme.density`.
- Текст раскладывается один раз в `hey-text`; рендереры получают готовые глифы (ADR-5).
- CPU-рендерер — эталон; будущий GPU-бэкенд сверяется с ним golden-тестами (ADR-6).

## Тесты

- Юнит-тесты в крейтах, интеграционные — `crates/*/tests` (виджеты, overlay, фокус, reconcile, платформа).
- Golden-снимки CPU-рендерера: `crates/hey-render-cpu/tests/golden.rs`.
- CI: `fmt --check`, `clippy -D warnings`, `cargo test --workspace` (Linux + Windows).

## Примеры

`examples/counter`, `todo`, `gallery` — на публичном API; `prototype_monolith` — исторический монолит для сверки, не развивается.
