# Roadmap after Phase 10

Phases 0-9 are closed and Phase 10 is mostly done (see `DEVELOPMENT_PLAN.md`). This document orders the remaining work into releases. Versions follow `API_POLICY.md`: a breaking change bumps `0.y`.

## Measured targets (Windows, release, `examples/counter`)

| Target | Goal | Result |
|---|---|---|
| Input to frame, 1080p, CPU backend | <= 16 ms | 8.6 ms (typical UI) |
| Idle redraws | none | 1 frame in 14 s (event loop is `ControlFlow::Wait`) |
| Idle CPU | ~0% | 0.5-2.5% in `Process.TotalProcessorTime` samples; no redraws, so this is OS-side message and accessibility overhead |
| GPU frame, 2000 rects incl. readback (dev machine, `cargo bench -p hey-render-gpu`) | n/a | 4.1 ms at 1080p, 14.6 ms at 4K |
| Cold start to visible window | < 300 ms | 137-146 ms warm; 363 ms on the first run after a build (cold disk cache) |

Open measurement gaps: Linux and macOS numbers and a first-frame marker (the window-handle probe above is approximate).

## 0.1.0: release gate

Goal: the first publishable version.

Deferred by the owner (2026-11-26): crates.io work (name check, `cargo package` CI gate, publishing). Development continues with 0.2; these items stay open.

- [ ] API review using the checklist in `API_POLICY.md`:
  - `#![warn(missing_docs)]` in every public crate, then fix the warnings (done: i18n, icons, render-cpu, render-gpu; backlog ~500 items in the other seven).
  - [x] `#[non_exhaustive]` on `UiEvent`, `DrawCmd`, `Role` and `GpuError`.
  - [x] Typed errors: `hey_shell::Error`, `hey_i18n::I18nError`.
  - [x] No `unwrap` in library code outside tests (shell runtime failures no longer panic; the remaining `expect` calls document internal invariants).
  - [x] No `taffy` / `winit` / `wgpu` / `fontdue` types in public signatures unless intended (only `GpuRenderer::new_for_window` takes a wgpu surface target, on purpose).
- [ ] Check that the `hey-*` names are free on crates.io; reserve them if not.
- [ ] CI: add macOS to the matrix; add a `cargo package` check and a `cargo doc` warnings gate.
- [ ] Publish in dependency order: core, draw, layout, style, text, i18n, icons, render-cpu, render-gpu, widgets, shell, test. Needs owner approval and a crates.io token.
- [ ] Manual QA that tests cannot cover: Ctrl+Shift+F12, minimize/restore, `HEY_VSYNC=0`, IME, a screen reader, `auto_theme` with a real OS switch, macOS Cmd shortcuts.

## 0.2: everyday widgets and input

Goal: enough to build a real application.

- [x] Scroll containers (`Style::scroll`): clipping, wheel bubbling, draggable scrollbar thumb, track paging.
- [x] `List::add_keyed`: selection and scroll follow the row key across view rebuilds.
- [x] `TextInput::add_submit`: Enter callback.
- [x] DPI scale factor end to end: layout and events in logical units, `Ui::paint` returns physical pixels (`Ui::set_scale`, `DisplayList::scale_by`).
- [x] Theme loading from disk and hot reload (`Theme::from_file`, `App::watch_theme`).
- [x] Hover/press/focus crossfade for every widget (`Ui::tick`, `Ui::animating`, `DisplayList::blend`); the shell redraws every 16 ms while it runs. Off with reduced motion.
- [x] Slice sizes: `themes.json` is the source of truth per theme (8/16/18, zeta-remastered 4/8/14).

## 0.3: platform

Goal: native feel on every OS. Needs hardware or CI runners per OS.

- [x] Windows: DWM dark title bar follows the app theme (`ShellApp::titlebar_dark`, applied at window creation and on system theme change). A DPI manifest is not needed: winit enables per-monitor DPI awareness itself.
- [ ] Manual QA: dark title bar on a light OS theme (set a dark app theme, check the caption).
- [ ] macOS: native menu bar.
- [~] Linux: `rfd` (gtk3) backend compiles; clipboard verified on Wayland and X11 (WSLg) with `arboard` `wayland-data-control`. Not verified by hand: native file dialogs and drag and drop (winit delivers `DroppedFile` on both).
- [x] Native queries for high contrast and reduced motion: done on Windows (`SystemParametersInfoW`); macOS and Linux still use the `HEY_HIGH_CONTRAST` / `HEY_REDUCE_MOTION` env flags.
- [~] Multi-window: `hey-shell` routes events per window (`WindowKey`, `take_new_windows`, `window_closed`; example `multi_window`). The MVU `App` runtime does not use it yet; only checked by build, not by hand.

## 0.4: text

Goal: correct text in every script.

- [x] Real shaping via `harfrust` inside `hey-text/layout.rs` (kerning, ligatures, Arabic joining; ADR-8). Known limits: no shaping across whitespace, `y_offset` ignored.
- [x] Bidi mirroring of brackets in RTL runs (leading neutrals take the next strong direction).
- [x] Cluster-based caret/selection in ligatures (caret and hit-testing snap to grapheme boundaries inside a ligature glyph).
- [x] Golden tests for RTL and complex scripts (`text_rtl`: Arabic joining, Hebrew, mixed directions, mirrored brackets) on bundled Noto Sans Arabic/Hebrew.

## Quality track (any release)

- [x] `cargo-fuzz` targets for the text editor and arena (`fuzz/`, nightly, run with `cargo +nightly fuzz run text_edit`).
- [x] GPU frame benchmark (`crates/hey-render-gpu/benches/gpu_frame.rs`).
- [x] `tracing` spans (`frame`, `app_draw`, `render`, `ui_paint`, `layout`) behind the optional `hey-shell/profiling` feature; Tracy or puffin attach as a `tracing` layer.
- [x] Optimise the rect rasterizer: interior pixels skip per-pixel coverage (2000 bordered chamfered rects: 70 ms to 32 ms at 1080p, 301 ms to 80 ms at 4K; output unchanged).
- [x] Linux performance baseline (WSL2 Ubuntu, Ryzen 9 5950X, release; see `DEVELOPMENT_PLAN.md`). macOS baseline still needs a Mac.

## Differentiators track (post-0.1)

Hey's unusual base is a pure display list, a byte-deterministic CPU rasterizer and a headless harness. Every item below builds on that; none needs a new renderer.

1. **Session replay (`hey-test`).** Record the timestamped `UiEvent` stream plus the app messages to a small text file. `Harness::replay(file)` reproduces a user's bug headlessly and bit-exactly, and a recording can be promoted to a golden test. Acceptance: record in the shell with a hotkey, replay in CI, identical frame hashes on Windows and Linux.
2. **Time-travel debugging.** The MVU `Msg` log is already a replayable history; add a scrubber overlay (dev builds) that rewinds the model and re-renders any frame. Acceptance: step back 100 messages without side effects (commands are replayed from the log, not re-run).
3. **Display-list inspector.** Serialize `DrawCmd` lists and the layout tree; an overlay shows the widget under the cursor, its rect, style and a11y role, and diffs two frames. Acceptance: toggled by Ctrl+Shift+F12, zero cost when off.
4. **Accessibility gates as tests.** Contrast audit of themes (done: `Theme::audit_contrast`), then tree lints in `hey-test`: every interactive node has a name, minimum 24 logical px targets, a keyboard path reaches every control, focus order matches reading order. Acceptance: `Harness::audit()` returns a list; the cookbook example fails CI on a violation.
5. **Text-scale and locale matrix tests.** Run one golden scenario at 100/150/200% text scale and in LTR/RTL, and assert nothing clips or overlaps (layout overlap checker over the tree). Acceptance: the matrix is one helper call.
6. **Pseudo-localization.** `I18n::pseudo()` expands strings by about 40% with accents and brackets, to find truncation before translators do. Acceptance: toggled by `HEY_PSEUDO_LOCALE=1`.
7. **Design tokens as a contract.** `themes.json` becomes validated on load (missing tokens, contrast, density ranges) and exported to CSS/Figma variables so design and code cannot drift. Acceptance: `hey-style` CLI check in CI.
8. **Deterministic cross-platform screenshots as an API.** `Ui::render_png` without a window, for docs, bug reports and social cards. Acceptance: the site screenshots are generated by a test.
9. **MVU multi-window and docking.** Per-window view functions in `App`, then dockable panels built on the multi-window shell. Acceptance: `multi_window` example moves to the MVU API.
10. **Core polish.** macOS menu bar and text-scale reader, `WindowConfig::min_size`, light-theme accent contrast (see `builtin_themes_pass_contrast_audit`), vello backend evaluation.

Order: 4 (started), 1, 3, 5, 6, 7, 8, 2, 9, 10.

## 1.0 criteria

- The review checklist passes for `hey-core`, `hey-style`, `hey-layout`, `hey-widgets`, `hey-shell`.
- All 0.2 items done; platform items done or explicitly documented as unsupported.
- Two consecutive minor releases without breaking changes in the Public tier.

## After 1.0

See Phase 11 in `DEVELOPMENT_PLAN.md`: vello backend, reactive signals and a `ui!{}` macro, devtools inspector, web and mobile, docking.
