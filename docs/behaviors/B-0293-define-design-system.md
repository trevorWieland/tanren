---
schema: tanren.behavior.v0
id: B-0293
title: Define and maintain a cross-surface design system
area: experience-design
personas: [solo-builder, team-builder]
surfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can define and maintain shared design
principles, tokens, vocabulary, patterns, accessibility expectations, and
surface adapters, so human and agent-generated work remains coherent across
project surfaces.

## Preconditions

- A product brief or planning context exists.
- A surface registry exists.
- The user has permission to edit experience planning context.

## Observable outcomes

- The user can define reusable design tokens, vocabulary terms, pattern IDs,
  accessibility expectations, and surface adapters.
- Design-system pattern IDs are stable and portable: roadmap nodes,
  experience contracts, proof artifacts, and walk records can reference them.
- The design system can describe non-web surfaces such as CLI, TUI, API, MCP,
  gameplay, SDK, chat, desktop, mobile, or embedded surfaces without requiring
  a web component model.
- Design-system changes preserve history and mark downstream experience
  contracts or roadmap nodes stale when the change can affect generated work.

## Out of scope

- Defining project surfaces. Use `define-surfaces`.
- Designing per-behavior flows and states. Use `design-experience`.
- Choosing implementation architecture or component frameworks. Use
  `architect-system`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.

## Related

- B-0289
- B-0290
- B-0294
