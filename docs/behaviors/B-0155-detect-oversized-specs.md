---
schema: tanren.behavior.v0
id: B-0155
title: Detect oversized specs and propose splits
area: spec-quality
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can detect oversized specs and review proposed splits so work stays small enough to shape, execute, and walk safely.

## Preconditions

- A draft or shaped spec exists.
- The user has permission to edit the spec or create related specs.

## Observable outcomes

- Tanren explains why the spec appears too large or cross-cutting.
- Proposed split specs preserve links to the original intent.
- The user can accept, revise, or reject the proposed split.

## Out of scope

- Splitting specs solely around code modules.
- Creating new specs without preserving source context.

## Related

- B-0077
- B-0078
- B-0110
