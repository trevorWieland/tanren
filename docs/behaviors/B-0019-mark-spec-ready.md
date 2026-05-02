---
schema: tanren.behavior.v0
id: B-0019
title: Mark a spec ready to run
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can mark a shaped spec as ready to run, so that an
implementation loop may be started on it (manually via B-0001 or automatically
via B-0002).

## Preconditions

- The spec exists in the project and has been shaped (B-0018).
- The user has permission to change the spec's lifecycle state.

## Observable outcomes

- The spec moves from draft to ready, visible via B-0021.
- Only ready specs are eligible to have an implementation loop started on
  them.
- The user can move a ready spec back to draft if further shaping is needed.

## Out of scope

- Deciding *which* ready spec runs next — prioritization is covered by B-0020.
- Starting the loop itself — covered by B-0001.

## Related

- B-0001
- B-0018
- B-0020
- B-0021
