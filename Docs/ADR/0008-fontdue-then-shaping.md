# ADR-8: Text: `fontdue` to start, then a shaping engine

**Status:** Partially accepted

## Decision

Rasterization and metrics currently use `fontdue` (Cyrillic works). Replacing it with `cosmic-text`/`parley` (shaping, bidi, fallback, IME) will be decided based on a spike.

## Rationale

A quick start without a heavy dependency; complex scripts will require shaping.

## Consequences

The spike is not done yet (`DEVELOPMENT_PLAN.md`, phase 2). Isolating text in `hey-text` (ADR-5) keeps the cost of replacement low.
