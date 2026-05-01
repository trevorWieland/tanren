---
schema: tanren.behavior.v0
id: B-0167
title: Show expected versus actual behavior
area: walk-acceptance
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can compare expected and actual behavior during review so acceptance decisions are grounded in observed outcomes.

## Preconditions

- A spec has acceptance criteria or expected outcomes.
- Walk, test, demo, or review source signals exist.

## Observable outcomes

- Expected outcomes are shown next to observed results.
- Differences are identified as accepted, unresolved, or routed to follow-up.
- Supporting source references remains reachable from the comparison.

## Out of scope

- Requiring a specific testing framework.
- Claiming behavior passed when source signals are absent.

## Related

- B-0072
- B-0076
- B-0166
