---
schema: tanren.behavior.v0
id: B-0084
title: Configure organization standards policy
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with policy permission can configure organization standards
profile requirements so projects inherit consistent quality expectations without
overwriting project-authored standards silently.

## Preconditions

- The active organization exists.
- The user has permission to manage standards policy.

## Observable outcomes

- The organization records required or default standards profiles.
- Projects can see which standards requirements apply.
- Project configuration cannot silently ignore mandatory standards.

## Out of scope

- Authoring the standards themselves.
- Project-specific standards edits.
- Remote standards distribution mechanics.

## Related

- B-0040
- B-0049
- B-0071
- B-0267
- B-0268
