# ADR-3: Coordinates are physical pixels

**Status:** Accepted

## Decision

`DrawCmd`, layout and hit-testing work in physical pixels; scale is applied via `Theme.density`.

## Rationale

No mismatch between hit-testing and rendering, and no fractional logical coordinates at boundaries.

## Consequences

Window DPI scale handling is not done yet (it is on the deferred list): theme sizes must be scaled manually.
