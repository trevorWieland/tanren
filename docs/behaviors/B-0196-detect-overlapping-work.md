---
schema: tanren.behavior.v0
id: B-0196
title: Detect duplicate or overlapping work across builders
area: team-coordination
personas: [team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can detect duplicate or overlapping work across builders so effort does not diverge into conflicting or redundant outcomes.

## Preconditions

- Multiple visible work items, specs, graph nodes, or active loops exist.
- Tanren has enough source signals to compare their intent or affected surfaces.

## Observable outcomes

- Tanren identifies likely duplicate, overlapping, or conflicting work with rationale.
- Affected builders can see the relationship and decide whether to merge, split, sequence, or continue separately.
- False positives can be dismissed with a recorded rationale.

## Out of scope

- Blocking all parallel work.
- Comparing hidden work in a way that leaks its details.

## Related

- B-0013
- B-0124
- B-0191
