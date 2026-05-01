---
schema: tanren.behavior.v0
id: B-0158
title: Compare candidate work before prioritization
area: prioritization
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can compare candidate work before prioritization so choices are made against product value, risk, and effort.

## Preconditions

- Multiple roadmap items, specs, or candidate work items exist.
- The user has visibility into the compared work.

## Observable outcomes

- Tanren shows comparable dimensions such as mission alignment, urgency, risk, dependencies, and rough effort.
- Missing comparison source signals are called out.
- The comparison can support roadmap sequencing or deferral.

## Out of scope

- Pretending estimates are exact.
- Prioritizing hidden work the user cannot see.

## Related

- B-0092
- B-0098
- B-0159
