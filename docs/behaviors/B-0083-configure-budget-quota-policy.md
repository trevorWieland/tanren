---
schema: tanren.behavior.v0
id: B-0083
title: Configure organization budget and quota policy
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with policy permission can configure organization budgets and quotas so Tanren work stays within agreed resource limits.

## Preconditions

- The active organization exists.
- The user has permission to manage budgets or quotas.

## Observable outcomes

- The user can set limits for relevant work scopes.
- Users affected by limits can see the active limit and current status.
- Tanren blocks or pauses work when a configured limit requires it.

## Out of scope

- Billing implementation.
- Provider-specific metering internals.

## Related

- B-0040
- B-0034
