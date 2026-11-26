# Getting started with Hey

Hey is a declarative desktop UI framework in Rust. Your program is a Model–View–Update (MVU) loop:

```text
OS event -> widget -> Msg -> update(&mut Model, Msg) -> view(&Model) -> reconcile -> layout -> paint
```

## 1. Prerequisites

- Rust stable (edition 2021).
- Windows, Linux or macOS. The GPU backend needs a Vulkan/Metal/DX12 adapter; otherwise Hey falls back to the CPU renderer.
- The workspace is not published yet. Use a path or git dependency, or add your app under `examples/`.

## 2. A minimal app

This is a trimmed `examples/counter`:

```rust
use hey_core::{Callback, Insets};
use hey_layout::Style;
use hey_shell::{App, Command};
use hey_style::{Theme, Variant};
use hey_widgets::{Button, Label, View};

#[derive(Clone, Copy, Debug)]
enum Msg {
    Inc,
}

#[derive(Default)]
struct Model {
    n: i64,
}

fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Inc => m.n += 1,
    }
    Command::none()
}

fn view(m: &Model) -> Vec<View<Msg>> {
    let n = m.n;
    vec![View::container(
        "root",
        Style::column().padding(Insets::all(24.0)).gap(16.0),
        vec![
            View::leaf("value", n, move |ui, p| Label::add(ui, p, &format!("Value: {n}"))),
            View::leaf("inc", (), |ui, p| Button::add(ui, p, "+", Variant::Primary, Callback::new(|_| Msg::Inc))),
        ],
    )]
}

fn main() {
    let font = "design/fonts/onest/Onest[wght].ttf";
    App::new(Model::default()).update(update).view(view).theme(Theme::dark()).font(font).title("Counter").run().unwrap();
}
```

Run the bundled examples:

```text
cargo run -p counter
cargo run -p todo
cargo run -p gallery
```

## 3. Core ideas

- **`Model`** is plain data you own. **`Msg`** is an enum of things that can happen.
- **`update`** mutates the model and returns a `Command` (`none`, `msg`, `perform` for background work, `batch`).
- **`view`** is a pure function from the model to a tree of `View`s. Each node has a stable string key. The second argument of `View::leaf` is a props fingerprint: when it is equal between frames, the existing widget is kept.
- **Widgets** emit messages through `Callback`s; they never touch your model directly.
- **Layout** is flexbox (`Style::row()` / `Style::column()`, gap, padding) on top of `taffy`. Sizes are physical pixels.
- **Theme** comes from design tokens: `Theme::dark()`, `Theme::light()`, `Theme::high_contrast_dark()` / `high_contrast_light()`. `App::auto_theme(true)` follows the OS.

## 4. Renderers

The CPU renderer is the reference implementation. The GPU renderer (wgpu, SDF) is used when an adapter exists.

| Control | Effect |
|---|---|
| `HEY_RENDERER=cpu` or `gpu` | force a backend |
| `HEY_VSYNC=0` | disable vsync |
| `Ctrl+Shift+F12` | switch CPU/GPU at runtime |

## 5. Testing your UI

`hey-test` drives a `Ui` headlessly with simulated input and golden screenshots. See the [Cookbook](COOKBOOK.md#test-a-scenario).

## 6. Further reading

- [Cookbook](COOKBOOK.md): short recipes.
- [API policy](API_POLICY.md): semver, stability and review checklist.
- [Architecture](ARCHITECTURE.md), [Development plan](DEVELOPMENT_PLAN.md) and [Roadmap](ROADMAP.md).
- API reference: `cargo doc --workspace --no-deps --open`.
