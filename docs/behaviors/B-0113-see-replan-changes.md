---
schema: tanren.behavior.v0
id: B-0113
title: See when Tanren replans and what changed
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see when Tanren replans work and what changed so failures, conflicts, or new source signals do not silently alter the plan.

## Preconditions

- A plan exists and Tanren has produced a later plan revision.
- The user has visibility of the affected work.

## Observable outcomes

- The user can see that a replan occurred.
- The user can see the reason for replanning.
- The user can see added, removed, or changed work at a behavior level.

## Out of scope

- Exposing internal planner algorithms.
- Replanning without preserving history.

## Related

- B-0110
- B-0114
