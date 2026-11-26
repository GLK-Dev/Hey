# API policy: stability, semver and review

## Versioning

Hey follows [Semantic Versioning](https://semver.org/) with the usual pre-1.0 rule:

- `0.y.z`: a breaking change bumps `y`; compatible additions and fixes bump `z`.
- `1.0` is declared once the review checklist below passes for `hey-core`, `hey-style`, `hey-layout`, `hey-widgets` and `hey-shell`.
- All workspace crates are released together with the same version number.

Breaking means: removing or renaming a public item, changing a signature, adding a required trait method without a default, adding a variant to a public exhaustive enum, or tightening documented behaviour. Every such change needs a `CHANGELOG.md` entry under *Changed* or *Removed*.

## Minimum supported Rust version

The MSRV is the stable toolchain used by CI. Raising it is a minor-version change before 1.0 and is noted in the changelog.

## Stability tiers

| Tier | Crates | Promise |
|---|---|---|
| Public | `hey-shell`, `hey-widgets`, `hey-style`, `hey-layout`, `hey-core`, `hey-test`, `hey-i18n` | semver |
| Backend | `hey-draw`, `hey-render-cpu`, `hey-render-gpu`, `hey-text`, `hey-icons` | semver, but display-list commands and atlas internals may grow |

Items marked `#[doc(hidden)]` are not part of the API.

## Review checklist (before each release)

- [ ] `cargo doc --workspace --no-deps` has no warnings; every public item has docs.
- [ ] Public enums that may grow are `#[non_exhaustive]`.
- [ ] External crate types do not leak into public signatures unless intended (`taffy`, `winit`, `wgpu`, `fontdue`).
- [ ] Names follow the Rust API guidelines (`as_`/`to_`/`into_`, `new`/`with_`, no `get_` prefix).
- [ ] Builders take `self` and return `Self`; callbacks use `Callback`.
- [ ] Errors are handled consistently per crate; no `unwrap` in library code.
- [ ] Feature flags are additive.
- [ ] `cargo fmt`, `cargo clippy -D warnings` and `cargo test` pass on Linux and Windows.

## Known review items

- `App::run`, `App::build` and `hey_shell::run` return `hey_shell::Error`; `I18n::add_ftl` returns `I18nError` (both `#[non_exhaustive]`).
- `Widget` is a large trait; optional hooks (a11y, IME) may move to extension traits.
- `UiEvent`, `DrawCmd`, `Role` and `GpuError` are `#[non_exhaustive]`; downstream `match` needs a wildcard arm.
- `#![warn(missing_docs)]` is on for i18n, icons, render-cpu and render-gpu; the other crates are being documented one by one.
