---
schema: tanren.behavior.v0
id: B-0191
title: Resolve conflicting product direction
area: decision-memory
personas: [team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can resolve conflicting product direction so Tanren has one accepted planning context for future work.

## Preconditions

- Conflicting roadmap, mission, standards, spec, or decision context has been identified.
- The user has visibility into the conflicting context.

## Observable outcomes

- Tanren shows the conflict, affected work, alternatives, and supporting source references.
- The accepted resolution is recorded with rationale and attribution.
- Superseded direction remains available as historical context.

## Out of scope

- Making unresolved disagreement disappear from active planning.
- Encoding a specific voting or approval model into the behavior.

## Related

- B-0170
- B-0171
- B-0190
