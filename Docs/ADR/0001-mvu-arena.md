# ADR-1: MVU + arena; signals are an optional layer later

**Status:** Accepted

## Decision

The UI tree is stored in an arena, application state lives in `Model`, and changes flow through `Msg` and `update`.

## Rationale

Borrow-checker friendly (no `Rc<RefCell>` in the model), easier to debug and test: `update` is a pure function over the model. Reactive signals can be added on top without changing the core.

## Consequences

A diff (`Ui::reconcile`) is needed, along with carrying widget-local state across frames.
