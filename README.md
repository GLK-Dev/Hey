# Hey UI

**Website:** https://glk-dev.github.io/Hey/ | **Wiki:** https://glk-dev.github.io/Hey/wiki/

Declarative desktop UI framework in Rust with dual interchangeable backends (**CPU** via `tiny-skia`/`fontdue`/`softbuffer` and **GPU** via `wgpu` SDF rendering), built around an arena tree and unidirectional MVU data flow.

See [Docs/DEVELOPMENT_PLAN.md](Docs/DEVELOPMENT_PLAN.md) for the full phase-by-phase roadmap (`M0`-`M6`, ADRs 1-8, and crate boundaries) and [Thinks/idea.md](Thinks/idea.md) for the original research notes and monolithic prototype.

## Architecture Overview

- **Data flow:** `OS Event (winit) -> Input -> Action/Msg -> update(&mut Model) -> view(&Model) -> Reconcile -> Layout -> DrawCmd -> Backend`
- **Crates:** `hey-core`, `hey-draw`, `hey-layout`, `hey-text`, `hey-style`, `hey-widgets`, `hey-render-cpu`, `hey-render-gpu`, `hey-shell`, `hey-icons`, `hey-i18n`, `hey-test`
- **Current stage:** Phase 10 (quality and documentation); see the plan
- **Docs:** [Getting started](Docs/GETTING_STARTED.md), [Cookbook](Docs/COOKBOOK.md), [API policy](Docs/API_POLICY.md), [Changelog](CHANGELOG.md)

## Project Memory (VOS)

Registered in local VOS (`vos`) as project `hey`:

```powershell
vos continue hey --experimental
vos status hey --experimental
vos search "DrawCmd" --project hey
vos session start hey --note "Current milestone"
vos session stop hey --quality 5 --note "Next handoff"
```