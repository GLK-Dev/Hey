# Architecture Decision Records

- [ADR-1](0001-mvu-arena.md) — MVU + арена; сигналы — опциональный слой позже (Принято)
- [ADR-2](0002-node-id-generation.md) — `NodeId { index, generation }` (Принято)
- [ADR-3](0003-physical-pixels.md) — Координаты — физические пиксели (Принято)
- [ADR-4](0004-app-defined-msg.md) — Приложение определяет свой `Msg` (Принято)
- [ADR-5](0005-text-layout-once.md) — Текстовый layout делается один раз в `hey-text` (Принято)
- [ADR-6](0006-cpu-reference-first.md) — Сначала CPU-бэкенд как эталон (Принято)
- [ADR-7](0007-own-flex-layout.md) — Свой минимальный flex-layout; taffy — эталон для сверки (Принято (уточнено спайком))
- [ADR-8](0008-fontdue-then-shaping.md) — Текст: `fontdue` для старта, затем shaping-движок (Принято частично)
