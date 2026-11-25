# ADR-4: The application defines its own `Msg`

**Status:** Accepted

## Decision

The framework knows only `UiEvent` and `Callback<Msg, A>`; the message type parameterizes `Ui<Msg>`.

## Rationale

Removes the hardcoded `Action` of the prototype; widgets are reusable in any application.

## Consequences

Widgets are generic over `Msg`; callbacks are created in `view` via `Callback::new`.
