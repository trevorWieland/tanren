---
schema: tanren.behavior.v0
id: B-0194
title: Leave handoff notes for another builder
area: team-coordination
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can leave handoff notes for another builder so interrupted or transferred work keeps its human context.

## Preconditions

- The user has visibility into the work being handed off.
- The user has permission to add notes to the work context.

## Observable outcomes

- Handoff notes are linked to the relevant spec, loop, graph node, review, or planning item.
- The notes identify current status, unresolved questions, risks, and recommended next action.
- The receiving builder can find the handoff from the work item.

## Out of scope

- Replacing structured source signals with free-form notes.
- Exposing private notes outside their configured visibility.

## Related

- B-0011
- B-0187
- B-0193
