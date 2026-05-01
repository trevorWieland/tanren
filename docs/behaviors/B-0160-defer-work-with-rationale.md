---
schema: tanren.behavior.v0
id: B-0160
title: Defer work with an explicit rationale
area: prioritization
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can defer work with an explicit rationale so future planning knows why the work was not chosen.

## Preconditions

- Candidate, roadmap, or spec work exists.
- The user has permission to change prioritization state.

## Observable outcomes

- The deferred work remains visible outside active execution queues.
- The deferral reason is recorded and attributed.
- Tanren can resurface deferred work when its rationale becomes stale or contradicted.

## Out of scope

- Deleting deferred work.
- Treating deferral as permanent rejection.

## Related

- B-0020
- B-0173
- B-0177
