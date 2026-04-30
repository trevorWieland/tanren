---
schema: tanren.behavior.v0
id: B-0261
title: Deny machine clients across permission boundaries
area: integration-contract
personas: [integration-client, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can receive a clear permission denial when it crosses a boundary so machine access remains scoped and debuggable.

## Preconditions

- A client attempts an action outside its configured scope or permissions.

## Observable outcomes

- The request is denied without mutating Tanren state.
- The denial identifies the safe category of missing permission, scope, or policy constraint.
- The denial is recorded for audit and does not reveal hidden resources.

## Out of scope

- Treating machine clients as trusted because they are internal automation.
- Returning hidden resource details to explain denial.

## Related

- B-0221
- B-0239
- B-0256
