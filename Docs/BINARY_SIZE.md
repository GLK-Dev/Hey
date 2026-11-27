# Binary size

An application built on Hey only links what it uses. Two things decide the size: the cargo features of `hey-shell` and the release profile of **your** application (a `[profile.release]` in the Hey workspace does not apply to your build).

## Measured (Windows, `counter` example)

| Configuration | Size |
|---|---|
| Default features, no release profile | 7.42 MB |
| Default features, size profile below | 5.21 MB |
| `default-features = false` (CPU renderer only), size profile | 2.04 MB |

## Features of `hey-shell`

| Feature | Default | Effect |
|---|---|---|
| `gpu` | on | wgpu renderer (`HEY_RENDERER=gpu`, F12 switch). Off: CPU renderer only, no wgpu in the binary. |
| `profiling` | off | `tracing` spans. |

```toml
[dependencies]
hey-shell = { version = "0.1", default-features = false }
```

With `gpu` off, `HEY_RENDERER=gpu` falls back to the CPU renderer with a message on stderr.

## Recommended release profile

```toml
[profile.release]
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

`panic = "abort"` removes unwinding tables; do not use it if you rely on `catch_unwind`.

## Not done yet

Finer features for dialogs (`rfd`), clipboard (`arboard`), accessibility (`accesskit`) and the bundled font, and measurements of `opt-level = "s"`.
