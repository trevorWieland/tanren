---
id: B-0081
title: Configure organization runtime placement policy
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with policy permission can configure where organization work is allowed to run so that execution placement follows governance requirements.

## Preconditions

- The active organization exists.
- The user has permission to manage runtime placement policy.

## Observable outcomes

- The policy names allowed or disallowed execution target classes.
- Projects in the organization can see which placement rules affect them.
- Tanren blocks work placement that violates the policy.

## Out of scope

- Provisioning execution targets.
- Cross-organization placement rules.

## Related

- B-0040
- B-0102
- B-0108
