---
schema: tanren.behavior.v0
id: B-0281
title: Maintain the system architecture record
area: architecture-planning
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can maintain the system architecture record
so accepted behaviors have an explicit design posture before implementation
work is planned.

## Preconditions

- A product brief and behavior catalog exist.
- The user has permission to edit architecture planning context.

## Observable outcomes

- The architecture record captures product-relevant boundaries, tradeoffs,
  operational posture, and subsystem responsibilities.
- Architecture changes remain attributable and reviewable.
- The architecture record distinguishes accepted direction from open questions
  or uncertain assumptions.

## Out of scope

- Replacing implementation tasks or specs.
- Embedding private implementation source signals in behavior files.
- Treating architecture decisions as proof that behavior is implemented.

## Related

- B-0276
- B-0282
- B-0283
