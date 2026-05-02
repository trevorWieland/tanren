---
schema: tanren.behavior.v0
id: B-0040
title: Configure organization approval policy
area: governance
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` who holds the permission to manage organization policy can
configure approval requirements for organization-owned work so that sensitive
actions follow consistent human-review rules.

## Preconditions

- The user has permission to manage organization approval policy.
- The context is organizational; this behavior does not apply to personal
  projects.

## Observable outcomes

- The user can require approval before selected actions proceed, such as
  starting work, accepting a walk, merging work, or changing sensitive
  configuration.
- The user can see which approval rules apply across the organization.
- Approval policy is visible to members affected by it.
- Project-level configuration cannot weaken an organization approval rule.

## Out of scope

- Runtime placement policy.
- Harness allowlist policy.
- Budget and quota policy.
- Standards policy.
- Cross-organization policies.

## Related

- B-0012
- B-0031
- B-0038
- B-0081
- B-0082
- B-0083
- B-0084
- B-0085
