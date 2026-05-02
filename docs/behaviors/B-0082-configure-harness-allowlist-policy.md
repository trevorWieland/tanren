---
schema: tanren.behavior.v0
id: B-0082
title: Configure organization harness allowlist policy
area: governance
personas: [team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with policy permission can configure which code harnesses organization projects may use so that execution uses approved agent providers only.

## Preconditions

- The active organization exists.
- The user has permission to manage harness policy.

## Observable outcomes

- The organization records allowed and blocked harnesses.
- Project harness choices cannot violate the organization allowlist.
- Users can see why a harness is unavailable when policy blocks it.

## Out of scope

- Managing user credentials for a harness.
- Installing a harness provider.

## Related

- B-0040
- B-0099
- B-0100
