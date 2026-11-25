# ADR-5: Text layout is done once in `hey-text`

**Status:** Accepted

## Decision

Glyph positions are computed by `hey-text`; both backends consume ready-made glyphs.

## Rationale

CPU/GPU parity and shared metrics for caret and selection hit-testing.

## Consequences

The renderer cannot lay out text itself; swapping the shaper (ADR-8) affects only `hey-text`.
