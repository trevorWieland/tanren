---
id: B-0167
title: Show expected versus actual behavior
area: walk-evidence
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can compare expected and actual behavior during review so acceptance decisions are grounded in observed outcomes.

## Preconditions

- A spec has acceptance criteria or expected outcomes.
- Walk, test, demo, or review evidence exists.

## Observable outcomes

- Expected outcomes are shown next to observed results.
- Differences are identified as accepted, unresolved, or routed to follow-up.
- Supporting evidence remains reachable from the comparison.

## Out of scope

- Requiring a specific testing framework.
- Claiming behavior passed when evidence is absent.

## Related

- B-0072
- B-0076
- B-0166
