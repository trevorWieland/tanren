---
schema: tanren.behavior.v0
id: B-0154
title: Identify missing edge cases in a spec
area: spec-quality
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can see likely missing edge cases in a spec so acceptance criteria cover behavior users care about.

## Preconditions

- A draft or shaped spec exists.
- The user has visibility into the spec and its product context.

## Observable outcomes

- Tanren calls out likely missing edge cases with rationale.
- The user can accept, revise, defer, or reject each suggested edge case.
- Accepted edge cases can become acceptance criteria or follow-up work.

## Out of scope

- Guaranteeing exhaustive edge-case discovery.
- Forcing every suggested edge case into the current spec.

## Related

- B-0076
- B-0153
- B-0155
