# ADR-8: Text: `fontdue` to start, then a shaping engine

**Status:** Accepted (spike done; migration deferred)

## Decision

Rasterization and metrics currently use `fontdue` (Cyrillic works). `cosmic-text` is the chosen replacement when shaping, bidi or complex scripts are needed. The migration happens entirely inside `hey-text`, not before such a need appears.

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
