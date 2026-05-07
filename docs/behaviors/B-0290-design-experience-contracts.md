---
schema: tanren.behavior.v0
id: B-0290
title: Design experience contracts for behavior-surface pairs
area: experience-design
personas: [solo-builder, team-builder]
surfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can produce experience contracts — entry
points, primary flows, observable states, copy and accessibility
expectations, and proof artifacts — for each accepted behavior on each
surface that behavior declares, so implementation and proof obligations are
known before roadmap work is shaped.

## Preconditions

- A product brief or planning context exists.
- An accepted behavior catalog and an active surface registry exist.
- The user has permission to edit experience planning context.

## Observable outcomes

- The user can record an experience contract for any behavior-surface pair,
  covering at minimum: entry point, primary flow, success state, failure
  states, empty/loading/stale/permission-denied/unavailable states, and the
  proof artifact required to make the behavior reviewable on that surface.
- Recorded experience contracts are reviewable independently of code: a
  reviewer can assess entry, flow, states, and proof obligations without
  reading implementation source.
- Behaviors with no recorded contract on a declared surface are explicitly
  visible as gaps; deliberate omissions carry a recorded rationale.
- Updating a behavior or a surface flags downstream contracts that may now be
  stale.

## Out of scope

- Choosing implementation architecture or framework. Use `architect-system`.
- Adding or removing accepted behaviors. Use `identify-behaviors`.
- Defining the surface registry itself. Use `define-surfaces`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.

## Related

- B-0289
- B-0291
- B-0292
