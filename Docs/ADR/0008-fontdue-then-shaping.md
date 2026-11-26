# ADR-8: Text: `fontdue` to start, then a shaping engine

**Status:** Accepted (amended: shaping done with `harfrust`)

## Decision

Rasterization and metrics currently use `fontdue` (Cyrillic works). `cosmic-text` is the chosen replacement when shaping, bidi or complex scripts are needed. The migration happens entirely inside `hey-text`, not before such a need appears.

## Amendment

Instead of `cosmic-text` + `swash`, shaping is done by `harfrust` inside `hey-text/src/layout.rs`. Whitespace-delimited chunks are shaped per font/direction run; glyphs are mapped to graphemes by cluster. Our own wrapping, bidi, fallback, ellipsis and hit-testing stay, and rasterization stays on `fontdue` by glyph id, so no `swash` is needed. `cosmic-text` remains only as a spike dev-dependency. Limits: no shaping across whitespace, `y_offset` ignored. A caret inside a ligature is interpolated across the glyph by byte fraction.

## Rationale

A quick start without a heavy dependency. The spike (`crates/hey-text/tests/cosmic_spike.rs`, `cosmic-text` 0.19 as a dev-dependency) showed:

- Latin and Cyrillic widths match `fontdue` within 1% (Onest, 16 px), so layouts will not shift noticeably.
- Bidi works: in `abc שלום def` the Hebrew run gets an RTL embedding level; Arabic is shaped (joined forms).
- Fallback fonts work: emoji sequences and combining marks are shaped with a second font.
- Word wrapping honors the max width.
- Cost: `FontSystem::new()` with system fonts takes ~27 ms (debug) and shaping is ~0.3 ms per short line, which is fine with a shaped-line cache.

## Consequences

- Swapping the shaper affects only `hey-text` (ADR-5); `PlacedGlyph` and `GlyphRun` stay the same.
- Still to do at migration time: glyph rasterization via `swash` (`SwashCache`) instead of `fontdue` bitmaps, mapping caret/selection hit-testing onto cosmic-text clusters, and IME support.
- `TextEditState` (graphemes, words, undo) is independent of the shaper and stays as is.
