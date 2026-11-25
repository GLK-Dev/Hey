# ADR-6: CPU backend first, as the reference

**Status:** Accepted

## Decision

`CpuRenderer` is implemented first; the GPU backend will be checked against it with golden tests with a tolerance.

## Rationale

Easier to debug and to catch regressions; anti-aliasing differences are pinned down by rules (a border is centered on the edge).

## Consequences

The GPU backend (`hey-render-gpu`) does not exist yet; the comparison tolerance will be defined when it appears.
