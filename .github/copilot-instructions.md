# Hey UI — Copilot Instructions

Follow `AGENTS.md` and `Docs/DEVELOPMENT_PLAN.md` strictly:
- Enforce crate boundaries (`hey-core` <- `hey-layout`/`hey-style`/`hey-text`/`hey-draw` <- `hey-widgets` <- `hey-shell`); keep core/layout/draw/widgets independent of `winit`/`wgpu`/`tiny-skia`.
- Pass `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`; no `.unwrap()`/`.expect()` outside tests.
- Recover project context via VOS (`vos continue hey --experimental`, `vos search "<query>" --project hey`, `vos status hey --experimental`).