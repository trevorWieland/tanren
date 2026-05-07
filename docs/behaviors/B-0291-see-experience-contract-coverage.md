---
schema: tanren.behavior.v0
id: B-0291
title: See experience contract coverage across behaviors and surfaces
area: experience-design
personas: [solo-builder, team-builder, observer]
surfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `observer` can see which behavior-surface
pairs have a recorded experience contract, which are explicitly out of scope,
and which are unaddressed gaps, so planning and review can target the work
that actually changes user-visible reach.

## Preconditions

- An accepted behavior catalog and an active surface registry exist.

## Observable outcomes

- The user can see, for every accepted behavior, the matrix of declared
  surfaces and the experience-contract state of each pair: contracted, in
  draft, gap, or explicitly out of scope.
- The user can filter by surface, by behavior area, by experience risk, or
  by gap state, so the view scales beyond the full catalog.
- The user can see when an experience contract was last updated relative to
  its behavior or surface, so stale contracts are detectable without reading
  every record.
- Coverage state is observable through Tanren's read paths and is consistent
  with what `craft-roadmap` and `design-experience` actually wrote.

## Out of scope

- Editing experience contracts. Use `design-experience`.
- Editing the surface registry. Use `define-surfaces`.
- Producing executable behavior proof. That is `B-0285` and per-node
  `expected_evidence`.

## Related

- B-0277
- B-0289
- B-0290
