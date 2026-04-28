---
id: B-0180
title: Link shipped changes back to roadmap and specs
area: release-learning
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can link shipped changes back to roadmap and specs so product progress remains traceable after delivery.

## Preconditions

- Work has shipped, merged, or otherwise been marked delivered.
- The user has visibility into the shipped work and related planning context.

## Observable outcomes

- Shipped changes link to their source specs, roadmap items, and evidence.
- The roadmap can distinguish planned, in-progress, shipped, and superseded work.
- Missing links are visible for correction.

## Out of scope

- Requiring an external release system.
- Treating merge as the only possible definition of shipped.

## Related

- B-0092
- B-0116
- B-0178
