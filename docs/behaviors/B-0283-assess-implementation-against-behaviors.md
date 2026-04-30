---
schema: tanren.behavior.v0
id: B-0283
title: Assess implementation against accepted behaviors
area: implementation-assessment
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can assess current implementation against accepted behaviors so planning
starts from what appears true rather than from outdated assumptions.

## Preconditions

- A behavior catalog exists.
- The selected project has implementation, documentation, test, or source signals
  context available for assessment.

## Observable outcomes

- Accepted behaviors are classified as implemented, asserted, missing, stale,
  uncertain, or not yet assessed in a separate assessment view.
- Assessment rationale cites visible source signals without writing implementation
  details into behavior files.
- Roadmap and planning views can use assessment results without treating them as
  product acceptance.

## Out of scope

- Changing behavior `verification_status` without supporting source references.
- Treating static inspection as executable proof.
- Replacing human review when source signals are uncertain.

## Related

- B-0277
- B-0284
- B-0285
