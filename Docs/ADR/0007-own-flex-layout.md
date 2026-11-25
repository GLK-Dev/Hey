# ADR-7: Own minimal flex layout; taffy is the reference for cross-checking

**Status:** Accepted (refined by a spike)

## Decision

Row/Column/Stack layout with padding, gap and grow is implemented in `hey-layout`. The spike `tests/taffy_spike.rs` showed results matching `taffy` on a Row + gap + grow scenario.

## Rationale

Avoid blocking early phases on a third-party API and keep full control over physical pixels.

## Consequences

Migrating to `taffy` will be considered when grid/wrap is needed; until then `taffy` is used only for cross-checking.
