# Changelog

All notable changes are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/), and the project follows [Semantic Versioning](https://semver.org/) (see `Docs/API_POLICY.md`).

## [Unreleased]

### Added
- Workspace of 12 crates: core, layout (flexbox), style (design tokens, themes), text (shaping, bidi), draw (display list), CPU and GPU renderers, widgets, shell (winit), icons, i18n and the test harness.
- MVU application shell (`App`, `Command`, `View` reconciliation).
- Widgets: Button, Label, TextInput, TextArea, List, Tabs, Checkbox, Radio, Toggle, Slider, ProgressBar, Panel, Icon; overlays (Dropdown, Menu, Modal, Tooltip).
- Accessibility tree (AccessKit), IME support, Fluent i18n, RTL text.
- System appearance: light/dark, high contrast, reduced motion.
- Platform-aware shortcuts (Cmd on macOS) and Shift+Z redo.
- `hey-test`: input simulation and golden screenshots.
- Property tests (`proptest`) and `criterion` benchmarks.
- Documentation: getting started, cookbook, API policy.
- `Docs/ROADMAP.md`: release plan 0.1 to 1.0 and measured performance targets.
- MIT `LICENSE` and crates.io package metadata; `hey_text::DEFAULT_FONT` (bundled Onest).
- Scroll containers: `Style::scroll` containers clip children, take wheel events from any descendant, and draw a draggable scrollbar (thumb drag, track paging). `LayoutTree::parent`.
- `List::add_keyed`: stable row keys; selection (previously lost on rebuild) and scroll follow the key.
- `TextInput::add_submit`: `on_submit` callback fired by Enter.

### Changed
- `UiEvent`, `DrawCmd`, `Role` and `GpuError` are `#[non_exhaustive]`.
- `App::build`, `App::run` and `hey_shell::run` return `hey_shell::Error`; `I18n::add_ftl` returns `I18nError` (no more `String` errors, no winit type in the public API).
- `GpuRenderer::set_vsync` no longer returns a `wgpu` type. The shell skips a frame on surface errors and exits cleanly if the window cannot be created, instead of panicking.
- `missing_docs` enforced in hey-i18n, hey-icons, hey-render-cpu and hey-render-gpu.
