# Roadmap after Phase 10

Phases 0-9 are closed and Phase 10 is mostly done (see `DEVELOPMENT_PLAN.md`). This document orders the remaining work into releases. Versions follow `API_POLICY.md`: a breaking change bumps `0.y`.

## Measured targets (Windows, release, `examples/counter`)

| Target | Goal | Result |
|---|---|---|
| Input to frame, 1080p, CPU backend | <= 16 ms | 8.6 ms (typical UI) |
| Idle redraws | none | 1 frame in 14 s (event loop is `ControlFlow::Wait`) |
| Idle CPU | ~0% | 0.5-2.5% in `Process.TotalProcessorTime` samples; no redraws, so this is OS-side message and accessibility overhead |
| Cold start to visible window | < 300 ms | 137-146 ms warm; 363 ms on the first run after a build (cold disk cache) |

Open measurement gaps: Linux and macOS numbers, GPU frame time, and a first-frame marker (the window-handle probe above is approximate).

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
- [ ] `TextInput` Enter callback (submit).
- [ ] DPI scale factor end to end (layout in logical units, raster in physical).
- [ ] Theme loading from disk and hot reload; tick source for `Transition` redraws.
- [ ] Resolve the slice-size OPEN QUESTION in `DEVELOPMENT_PLAN.md` (`tokens.json` vs `themes.json`).

## 0.3: platform

Goal: native feel on every OS. Needs hardware or CI runners per OS.

- [ ] Windows: DPI awareness manifest, DWM titlebar.
- [ ] macOS: native menu bar.
- [ ] Linux: `rfd` backend, Wayland/X11 clipboard and drag and drop.
- [ ] Native queries for high contrast and reduced motion instead of env or heuristics.
- [ ] Multi-window.

## 0.4: text

Goal: correct text in every script.

- [ ] Migrate shaping to `cosmic-text` (a spike exists in `hey-text/tests/cosmic_spike.rs`).
- [ ] Bidi mirroring and Arabic shaping.
- [ ] Golden tests for RTL and complex scripts.

## Quality track (any release)

- [ ] `cargo-fuzz` targets for the text editor and arena.
- [ ] GPU frame benchmark; `tracing` with Tracy or puffin.
- [ ] Optimise the rect rasterizer (2000 bordered chamfered rects: 70 ms at 1080p).
- [ ] Linux and macOS performance baselines.

## 1.0 criteria

- The review checklist passes for `hey-core`, `hey-style`, `hey-layout`, `hey-widgets`, `hey-shell`.
- All 0.2 items done; platform items done or explicitly documented as unsupported.
- Two consecutive minor releases without breaking changes in the Public tier.

## After 1.0

See Phase 11 in `DEVELOPMENT_PLAN.md`: vello backend, reactive signals and a `ui!{}` macro, devtools inspector, web and mobile, docking.
