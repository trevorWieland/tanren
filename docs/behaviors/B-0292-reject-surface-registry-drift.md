---
schema: tanren.behavior.v0
id: B-0292
title: Reject behavior or roadmap drift from the project surface registry
area: experience-design
personas: [solo-builder, team-builder]
surfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can rely on Tanren to reject behavior
catalog or roadmap edits that reference an unknown surface ID, so the
surface registry stays the single source of truth for where behaviors are
reachable.

## Preconditions

- A surface registry exists.
- The user is editing behaviors, roadmap nodes, or scenario tags.

## Observable outcomes

- A behavior whose `surfaces:` includes an ID not in the active surface
  registry is rejected, with a message that names the offending behavior,
  the unknown surface ID, and the registry that defines the allowed set.
- A roadmap node whose `expected_evidence.surfaces` or `surface_scope`
  references an unknown surface is rejected with a message that names the
  offending node and the unknown ID.
- A BDD scenario tagged with an unknown surface ID is rejected, naming the
  feature file, the behavior, and the unknown tag.
- Removing a surface from the registry while behaviors, roadmap nodes, or
  scenario tags still reference it is reported as a drift error rather than
  silently dropping coverage.

## Out of scope

- Defining the surface registry. Use `define-surfaces`.
- Designing per-behavior experience contracts. Use `design-experience`.
- Producing the BDD evidence itself. That is per-node `expected_evidence`.

## Related

- B-0289
- B-0288
