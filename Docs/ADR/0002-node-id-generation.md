# ADR-2: `NodeId { index, generation }`

**Status:** Accepted

## Decision

Nodes are addressed by a pair of index and generation.

## Rationale

Safe removal of subtrees: a stale `NodeId` does not point to a foreign node that reused the arena slot.

## Consequences

Every access checks the generation; references to removed nodes yield `None` instead of a panic.
