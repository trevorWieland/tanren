---
schema: tanren.behavior.v0
id: B-0184
title: Continue the same work from another interface
area: cross-interface
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can continue the same project, spec, loop, or review from another interface so work is not trapped in the surface where it began.

## Preconditions

- The user has access to multiple public interfaces for the same scope.
- The target interface supports the desired behavior.

## Observable outcomes

- The target interface opens the same work context with current state.
- In-progress decisions, blockers, reviews, or source signals remain associated with the same work.
- Unsupported actions are explained rather than silently losing context.

## Out of scope

- Making laptop-only interfaces available on every device.
- Bypassing permissions through interface switching.

## Related

- B-0005
- B-0028
- B-0183
