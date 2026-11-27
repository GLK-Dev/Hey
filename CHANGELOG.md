# Changelog

All notable changes are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/), and the project follows [Semantic Versioning](https://semver.org/) (see `Docs/API_POLICY.md`).

## [Unreleased]

### Fixed
- Text scale applied after the first build (system `TextScaleFactor` at window creation, or a live change) left widgets sized for 100%: `Ui::reconcile` kept leaves whose props were unchanged. A size-affecting `set_theme` (font size or scale) now rebuilds leaves, with state and focus carried over; a dark/light switch still keeps them.

### Changed
- The light theme accent is darker (`#C47400`), so it passes the 3:1 contrast audit on the panel and keeps 4.5:1 for text on accent. `builtin_themes_pass_contrast_audit` now requires an empty result for the light theme; the site screenshots were regenerated.

### Added
- `WindowConfig::min_size` and `App::min_size(w, h)`: the smallest logical size the user can resize a window to.
- Dockable panels in `hey-widgets`: `Dock` (split tree of tabbed groups plus floating roots keyed by window number) with `select`, `close`, `move_panel(panel, target, DropZone)`, `float`, `dock_back` and `set_ratio`; `dock_view` renders a root as `View`s with the app's titles and content. Pure data, so it works with `window_view` and `Command::open_window` (example `docking`). Dragging is left to the app.
- MVU multi-window in `hey-shell`: `App::window_view(|key, &model| ...)` is the view function for every extra window, `Command::open_window(key, cfg)` / `Command::close_window(key)` open and close them from `update`, and `App::on_window_closed` turns a user-closed window into a message. `ShellApp::take_closed_windows` lets a runtime close windows. The `multi_window` example now uses this API.
- Time-travel debugging in `hey-shell`: `App::time_travel()` (needs `Model: Clone`, `Msg: Debug`) keeps a snapshot of the model after every message (capped at 1000, oldest dropped). `Runtime::seek(Seek::Back|Forward|Start|Live)`, `history_len`, `history_cursor` and `time_travel_seek` rewind and re-render any frame; `update` and commands are never re-run, because a snapshot is swapped in. While rewound, a status bar (`Ui::set_note`) shows the position, and incoming messages (including background task results) are dropped until `Seek::Live`. Hotkeys: Ctrl+Shift+Left/Right/Home/End.
- Headless screenshots: `Ui::render` / `Ui::render_png` draw the UI with the CPU renderer without a window (physical pixels, theme desktop colour, byte-identical on every platform). `hey-widgets` now depends on `hey-render-cpu`. The site screenshots (`site/assets/screenshot-*.png`) are generated and checked by `hey-test/tests/site_screenshots.rs` (`HEY_UPDATE_GOLDEN=1` rewrites them).
- Design tokens as a contract in `hey-style`: `validate_tokens` checks a `themes.json` (all colour tokens present and valid, chamfer/focus/motion ranges, `fastMs <= baseMs <= windowMs`, terminal palette, WCAG contrast) and returns `TokenIssue`s; `export_css` and `export_figma` write CSS custom properties (`--hey-color-text-muted`) and Design Tokens (DTCG) JSON; `Theme::to_css`/`to_design_tokens`. The `hey-style` binary (`check [file] [--against other.json]`, `export css|figma [file]`) runs in CI and fails if `design/tokens/themes.json` drifts from the built-in copy.
- Pseudo-localization in `hey-i18n`: `I18n::pseudo()`, `set_pseudo`, `is_pseudo` and `pseudo_localize` turn every translated message into accented text, about 40% longer, in brackets (`Save` becomes `[Šàʋé ~~]`) to find truncation early. `HEY_PSEUDO_LOCALE=1` enables it for every `I18n::new`. Letters are limited to glyphs the bundled Onest font has.
- Layout lints and a text-scale matrix in `hey-test`: `Harness::audit_layout` / `audit_layout` report children clipped by their parent and overlapping siblings (`LayoutIssue`, `LayoutIssueKind`); `Harness::text_scale_matrix(&[100, 150, 200], |h| ...)` reruns a check per text scale.
- Display-list inspector: `Ui::set_inspector` draws an overlay with the widget under the pointer (role, rect, state, style) and the command count; `inspector_snapshot`/`inspector_diff` compare frames, `dump_tree` and `DisplayList::dump`/`diff` give text output. The shell toggles it with Ctrl+Shift+F10 and pins a baseline with Ctrl+Shift+F9. Off by default, one `Option` check per frame.
- Live session capture in `hey-shell`: Ctrl+Shift+F11 starts and stops recording a window's input; the `.hey-session` file goes to `$HEY_SESSION_FILE` or `hey-<time>.hey-session` and replays with `Harness::replay_file`. The format (`Recording`, `Entry`, `ParseError`, `fnv1a`) moved to `hey_core::session`; `hey-test` re-exports it.
- Session replay in `hey-test`: `Harness::start_recording`/`stop_recording`/`advance`/`checkpoint`, `Recording` (line-based text format, `save`/`load`) and `Harness::replay`/`replay_file`. Checkpoints store an FNV-1a hash of the CPU frame, so a recorded bug replays headlessly and bit-exactly; `tests/sessions/typing.hey-session` runs on Windows and Linux CI.
- `Harness::audit` and `audit_tree` in `hey-test`: accessibility lints for unnamed, smaller than 24 px or keyboard-unreachable controls (`Issue`, `IssueKind`).
- `hey-shell` feature `gpu` (default on): build with `default-features = false` for a CPU-only binary without wgpu (counter: 5.2 MB to 2.0 MB with the release profile in `Cargo.toml`).
- `Theme::audit_contrast`, `contrast_ratio` and `relative_luminance` in `hey-style`: WCAG AA check of the colour pairs widgets draw. Known finding: the light theme accent on the panel is about 1.8:1.

## [0.1.0] - 2026-11-27

### Added
- System text scale: `Appearance::text_scale` (percent) and `Theme::with_text_scale`/`scale`/`font_px`. The shell reads it from Windows (`TextScaleFactor`), Linux (`QT_FONT_DPI`, GNOME `text-scaling-factor`) or `HEY_TEXT_SCALE`, re-reads it on window focus, and applies it even when `auto_theme` is off. The Onest font is unchanged; only the size factor changes. Spacing, `Icon`, `Slider`, `ProgressBar`, `TextInput` widths and the select chevron scale with it. At 100% rendering is byte-identical.
- Multi-window in `hey-shell`: `ShellApp::focus_window`, `take_new_windows`, `window_closed`, `WindowKey`/`MAIN_WINDOW`; each window has its own renderer, input and accessibility adapter. Example `multi_window`.
- Linux: clipboard works on pure Wayland sessions (`arboard` `wayland-data-control`); ignored `system_clipboard_roundtrip` test for desktop sessions.
- Optional `tracing` spans (`frame`, `app_draw`, `render`, `ui_paint`, `layout`); enable timings on stderr with `--features hey-shell/profiling` and `RUST_LOG`.
- GPU frame benchmark (`cargo bench -p hey-render-gpu`).
- `cargo-fuzz` targets `text_edit` and `arena` in `fuzz/` (separate workspace, nightly).
- RTL golden test `text_rtl` and bundled Noto Sans Arabic and Noto Sans Hebrew (OFL) in `design/fonts`; layout tests no longer depend on system Arial.
- Caret and selection hit-testing work inside ligatures: `PlacedGlyph` carries the cluster end, `caret_x` interpolates across the glyph and `byte_at_x` snaps to grapheme boundaries.
- Project site (`site/`, dark industrial landing page, logo, banner) and an mdBook wiki built from `Docs/`, deployed via GitHub Pages workflow.
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
- `LICENSE.md` (GLK-Dev Universal Innovation License, GUIL 1.0) and crates.io package metadata; `hey_text::DEFAULT_FONT` (bundled Onest).
- Scroll containers: `Style::scroll` containers clip children, take wheel events from any descendant, and draw a draggable scrollbar (thumb drag, track paging). `LayoutTree::parent`.
- `List::add_keyed`: stable row keys; selection (previously lost on rebuild) and scroll follow the key.
- `TextInput::add_submit`: `on_submit` callback fired by Enter.
- DPI scaling: `Ui::set_scale` / `Ui::scale`, `DisplayList::scale_by`. Layout and widget events are logical; pointer positions come in physical pixels and `paint` returns physical ones. The shell applies the window scale factor, including the IME and accessibility rectangles.
- `Theme::from_file` and `App::watch_theme`: load a theme from a `themes.json`-format file and hot-reload it on change (polled every 250 ms).
- State crossfade: hover, press and focus changes fade over `fast_ms` with the theme easing. `Ui::tick` / `Ui::animating`, `DisplayList::blend`; the shell drives them. Disabled by reduced motion.
- Windows: high contrast and reduced motion are read from the OS (`SystemParametersInfoW`); the env flags still force them on. New dependency: `windows-sys` (Windows only).
- CPU rasterizer: rectangle interiors take a fast path (about 2x faster at 1080p, 3.7x at 4K, pixel-identical).
- Golden test at 2x DPI (`scaled_2x`) and a cookbook section on scrolling and DPI.
- Windows: the title bar matches the app theme through `DwmSetWindowAttribute` (`ShellApp::titlebar_dark`, implemented by `App`). New `windows-sys` feature: `Win32_Graphics_Dwm`.
- `Harness::set_scale` in `hey-test`, with a 2x golden screenshot of a focused text input.

### Changed
- Brackets and other mirrored characters are mirrored in RTL runs, including at the start of a paragraph.
- Text is shaped with `harfrust` (kerning, ligatures, contextual alternates, Arabic joined forms); glyphs are still rasterized by `fontdue` by glyph id. Wrapping, bidi, fallback, ellipsis and hit-testing are unchanged. The text golden was regenerated because of kerning.
- **License:** MIT is replaced by GUIL 1.0 (`LICENSE.md`: non-commercial use, source must stay available). `Cargo.toml` uses `license-file` because GUIL is not an SPDX id. Releases already received under MIT stay MIT.
- `UiEvent`, `DrawCmd`, `Role` and `GpuError` are `#[non_exhaustive]`.
- `App::build`, `App::run` and `hey_shell::run` return `hey_shell::Error`; `I18n::add_ftl` returns `I18nError` (no more `String` errors, no winit type in the public API).
- `GpuRenderer::set_vsync` no longer returns a `wgpu` type. The shell skips a frame on surface errors and exits cleanly if the window cannot be created, instead of panicking.
- `ThemeError` is `#[non_exhaustive]` and gained an `Io` variant.
- Wheel deltas given to `Ui::event` are physical pixels, like pointer positions, and are divided by the scale. The shell scales wheel line steps by the window scale (`InputState::scale`).
- `missing_docs` enforced in hey-i18n, hey-icons, hey-render-cpu and hey-render-gpu.
