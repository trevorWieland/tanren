---
schema: tanren.behavior.v0
id: B-0177
title: Recover from a bad planning decision without losing history
area: undo-recovery
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can recover from a bad planning decision without losing history so product direction can change while preserving why the prior path was chosen.

## Preconditions

- A roadmap, graph, spec, or prioritization decision is now considered wrong.
- The user has permission to change the affected planning scope.

## Observable outcomes

- Tanren records why the plan is changing.
- Affected specs, graph nodes, and roadmap items can be revised, deferred, archived, or superseded.
- Prior decisions remain available as historical context.

## Out of scope

- Erasing accountability for prior decisions.
- Automatically cancelling in-flight work without applying configured policy.

## Related

- B-0113
- B-0160
- B-0174
- B-0170
