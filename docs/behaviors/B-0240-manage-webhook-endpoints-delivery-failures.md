---
id: B-0240
title: Manage webhook endpoints
area: integration-management
personas: [operator, integration-client]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can manage webhook endpoints so external automation receives Tanren
events only through configured subscriptions and scopes.

## Preconditions

- The user has permission to manage webhook delivery for the selected scope.

## Observable outcomes

- Webhook endpoints show scope, event categories, signing status, health, and recent delivery state.
- Failed deliveries can be retried, paused, disabled, or investigated according to policy.
- Secret signing material is not displayed after creation or rotation.

## Out of scope

- Remediating delivery failures after events have been attempted.
- Sending events outside the endpoint's configured scope.

## Related

- B-0128
- B-0229
- B-0242
- B-0275
