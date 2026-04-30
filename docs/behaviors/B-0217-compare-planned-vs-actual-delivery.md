---
schema: tanren.behavior.v0
id: B-0217
title: Compare planned versus actual delivery
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can compare planned versus actual delivery so schedule, scope, and outcome drift is visible.

## Preconditions

- Planned work and delivered or attempted work exist in the selected scope.
- The user has visibility into the compared planning and delivery source signals.

## Observable outcomes

- Tanren shows what shipped as planned, what changed, what slipped, and what was added or removed.
- Differences link to replans, decisions, blockers, or source signals where visible.
- The comparison can be scoped by roadmap item, milestone, initiative, project, or time window.

## Out of scope

- Treating plan changes as failures by default.
- Producing precise schedule judgments without planning source signals.

## Related

- B-0035
- B-0113
- B-0180
