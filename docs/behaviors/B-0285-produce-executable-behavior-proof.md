---
schema: tanren.behavior.v0
id: B-0285
title: Produce executable behavior proof
area: behavior-proof
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can produce executable behavior proof for an accepted behavior so
verification status reflects observable product source signals instead of
implementation claims.

## Preconditions

- An accepted behavior exists.
- The project has enough implementation surface to exercise the behavior.
- The user has permission to add or review behavior proof for the project.

## Observable outcomes

- The proof includes positive and falsification witnesses where applicable.
- Passing proof can support moving a behavior from implemented to asserted when
  behavior-proof policy is satisfied.
- Failed or incomplete proof shows what behavior proof is missing.

## Out of scope

- Treating unit tests of implementation details as behavior proof by default.
- Marking a behavior asserted without active executable behavior proof.
- Embedding proof artifacts inside the behavior file.

## Related

- B-0167
- B-0169
- B-0277
- B-0283
