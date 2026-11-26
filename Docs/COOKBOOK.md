# Hey cookbook

Short recipes. Imports are the ones from [Getting started](GETTING_STARTED.md).

## Run background work

`Command::perform` runs the closure on a worker thread and maps its result to a `Msg`.

```rust
fn update(m: &mut Model, msg: Msg) -> Command<Msg> {
    match msg {
        Msg::Load => Command::perform(|| std::fs::read_to_string("data.txt"), |r| Msg::Loaded(r.map_err(|e| e.to_string()))),
        Msg::Loaded(r) => {
            m.data = r;
            Command::none()
        }
    }
}
```

`TaskState::from_result` and `is_loading` help to model "loading / ready / failed" in the model.

## Run something at startup

```rust
App::new(model).update(update).view(view).init(Command::msg(Msg::Load)).run()
```

## Combine commands

`Command::batch([Command::msg(Msg::A), Command::perform(work, Msg::B)])`.

## Native dialogs and clipboard

`hey_shell::{open_file, open_files, save_file, pick_folder, confirm, message}` wrap `rfd`. The system clipboard is installed by the shell; text widgets use it for Ctrl/Cmd+C/X/V.

## Switch theme

`App::auto_theme(true)` follows the OS (light/dark, high contrast, reduced motion). To switch manually, call `Ui::set_theme`.

## Scrolling and DPI

Layout and widget events use logical pixels; `Ui::paint` returns physical ones (`Ui::set_scale`, set by the shell from the window). Raw input given to `Ui::event` is physical: pointer positions and `UiEvent::Wheel` deltas are divided by the scale. In your own input code, send a wheel line step as `40.0 * scale`, which is what the shell does. Scroll offsets (`LayoutTree::scroll`) stay logical.

## Overlays: menus, dropdowns, modals, tooltips

Widgets open overlays through `Ui::overlays()`. The stack handles outside-click dismissal and Escape; `Ui::trap_focus` keeps focus inside a modal. See `Dropdown`, `Menu`, `Modal` and `Tooltip` in `hey-widgets`.

## Accessibility

`Ui::a11y_tree()` builds an AccessKit tree from widget roles and labels, and `hey-shell` exposes it to the OS. Custom widgets provide a role by implementing `Widget::a11y`.

## Localisation

`hey-i18n` loads Fluent bundles and formats numbers and dates per locale. Text layout handles bidirectional (RTL) runs.

## Test a scenario

```rust
use hey_test::Harness;

#[test]
fn click_type_undo() {
    let mut h = Harness::<String>::new(320.0, 80.0);
    let root = h.ui.root();
    let input = TextInput::add(&mut h.ui, root, "", 240.0, Callback::new(|s| s));
    h.frame();
    h.click(input);
    h.type_text("Hello");
    assert_eq!(h.messages().last().unwrap(), "Hello");
    h.undo();
    h.screenshot(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden"), "after_undo");
}
```

The first run writes the golden PNG; later runs compare pixel by pixel. Set `HEY_UPDATE_GOLDEN=1` to refresh references. `Harness::command("z", shift)` uses Cmd on macOS and Ctrl elsewhere.

## Test update logic without a window

`App::build()` returns a `Runtime` with `pump()`, `frame(size)` and `model()`, so the whole MVU loop can run headlessly.

## Benchmarks

```text
cargo bench -p hey-layout --bench layout
cargo bench -p hey-widgets --bench frame
```
