---
schema: tanren.behavior.v0
id: B-0150
title: Propose standards updates from repeated findings
area: standards-evolution
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can receive proposed standards updates from repeated findings so recurring issues can become explicit working rules.

## Preconditions

- Findings or review feedback show a repeated pattern.
- The user has permission to edit project standards.

## Observable outcomes

- Tanren proposes a standards change with links to supporting findings.
- The user can accept, revise, or reject the proposed change.
- Accepted changes are traceable to the source signals that motivated them.

## Out of scope

- Automatically changing standards from a single finding.
- Hiding the cost or impact of a proposed standards change.

## Related

- B-0080
- B-0149
- B-0170
