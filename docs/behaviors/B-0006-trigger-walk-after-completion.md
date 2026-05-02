---
schema: tanren.behavior.v0
id: B-0006
title: Start a walk for implementation-ready work
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can start a walk for work that has passed its
implementation checks so that delivered behavior is reviewed before the spec is
considered done.

## Preconditions

- The spec's implementation work has completed.
- Required checks have passed.
- No readiness-blocking findings remain unresolved.

## Observable outcomes

- Starting the walk transitions the spec into a walk state visible to anyone
  with visibility of the spec.
- The walk becomes the explicit gate between implementation-ready work and done
  work.
- The user can see whether a walk is pending, in progress, or finished.
- A `solo-builder` can start a self-walk when no teammate review is required.

## Out of scope

- Reviewing delivered behavior during the walk.
- Accepting or rejecting walked work.
- Code review of the implementation — Tanren does not require users to review
  diffs in order to walk delivered behavior.

## Related

- B-0001
- B-0003
- B-0072
- B-0073
- B-0074
