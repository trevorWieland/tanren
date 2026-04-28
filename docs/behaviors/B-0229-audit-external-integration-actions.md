---
id: B-0229
title: Audit external actions performed through an integration
area: integration-management
personas: [observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can audit external actions Tanren performed through an integration so side effects outside Tanren remain accountable.

## Preconditions

- Tanren has performed visible actions through an external provider.
- The user has visibility into the action audit scope.

## Observable outcomes

- Audit records identify provider, connection ownership, actor, action category, target resource, time, and outcome.
- Failed, retried, and denied external actions remain visible.
- Secret values and hidden provider data are redacted.

## Out of scope

- Replacing provider-native audit logs.
- Hiding external side effects because they occurred outside Tanren.

## Related

- B-0042
- B-0133
- B-0242
