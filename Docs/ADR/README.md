# Architecture Decision Records

- [ADR-1](0001-mvu-arena.md) — MVU + arena; signals are an optional layer later (Accepted)
- [ADR-2](0002-node-id-generation.md) — `NodeId { index, generation }` (Accepted)
- [ADR-3](0003-physical-pixels.md) — Coordinates are physical pixels (Accepted)
- [ADR-4](0004-app-defined-msg.md) — The application defines its own `Msg` (Accepted)
- [ADR-5](0005-text-layout-once.md) — Text layout is done once in `hey-text` (Accepted)
- [ADR-6](0006-cpu-reference-first.md) — CPU backend first, as the reference (Accepted)
- [ADR-7](0007-own-flex-layout.md) — Own minimal flex layout; taffy is the reference for cross-checking (Accepted, refined by a spike)
- [ADR-8](0008-fontdue-then-shaping.md) — Text: `fontdue` to start, then a shaping engine (Partially accepted)
