---
schema: tanren.behavior.v0
id: B-0289
title: Define and maintain project surfaces
area: experience-design
personas: [solo-builder, team-builder]
surfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can declare, revise, and retire the public
human and machine-facing surfaces a project supports, so behavior, proof, and
generated work can be reasoned about without assuming a web-default product
shape.

## Preconditions

- A product brief or planning context exists.
- The user has permission to edit experience planning context.

## Observable outcomes

- The user can add a surface with a stable ID, kind, target devices, inputs,
  outputs, accessibility expectations, and proof artifact types.
- The user can revise or retire an existing surface with rationale, preserving
  history.
- The active surface registry is durable and portable: another tool, another
  command, or another contributor can read it and reach the same set of IDs
  the user authored.
- Surface IDs are stable; renaming or removing a surface that behaviors or
  roadmap nodes already reference is reported as a behavior change.

## Out of scope

- Designing per-behavior flows, states, or proof obligations. Use
  `design-experience`.
- Choosing implementation architecture or component frameworks for a surface.
  Use `architect-system`.
- Generating roadmap work for a surface. Use `craft-roadmap`.

## Related

- B-0276
- B-0290
- B-0292
