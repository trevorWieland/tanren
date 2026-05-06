---
schema: tanren.behavior.v0
id: B-0289
title: Session error taxonomy and observability
area: governance
personas: [solo-builder, team-builder, operator]
interfaces: [api]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

Session-related failures (missing session, corrupt session, session-store
read/write errors) are projected through the shared `AccountFailureReason`
taxonomy so every interface returns the same structured error codes. Session
errors are emitted through `tanren-observability` helpers rather than ad-hoc
`tracing::error!` calls in individual route handlers.

## Preconditions

- The API server is running with a valid backing store.

## Observable outcomes

- An unauthenticated request to a protected endpoint returns an
  `unauthenticated` failure code.
- Session store read failures return a `session_read_failed` failure code.
- Session install failures return a `session_install_failed` failure code.
- Session flush failures return a `session_flush_failed` failure code.
- All session errors are emitted through `tanren-observability` structured
  helpers, not raw `tracing::error!` calls.

## Out of scope

- Token refresh or session extension flows.
- Rate-limiting of unauthenticated requests.

## Related

- B-0043
- B-0046
