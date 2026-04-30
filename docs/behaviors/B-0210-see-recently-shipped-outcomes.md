---
schema: tanren.behavior.v0
id: B-0210
title: See recently shipped outcomes
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see recently shipped outcomes so completed work is visible as product progress, not just closed specs.

## Preconditions

- The selected scope has shipped, merged, released, or otherwise completed work.
- The user has visibility into the shipped outcomes.

## Observable outcomes

- Tanren summarizes shipped outcomes with links to roadmap items, specs, source signals, and release context.
- Outcomes distinguish user-visible change, internal improvement, fix, risk reduction, and follow-up work.
- The view makes clear when shipped work still has post-release follow-up pending.

## Out of scope

- Treating every merge as a product outcome.
- Publishing release notes.

## Related

- B-0178
- B-0180
- B-0211
