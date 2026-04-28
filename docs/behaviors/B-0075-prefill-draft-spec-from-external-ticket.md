---
id: B-0075
title: Prefill a draft spec from an external ticket
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can prefill a draft spec from an external ticket so that existing intake context becomes Tanren-owned work without manual re-entry.

## Preconditions

- An active project is selected.
- The project has an external tracker connected for intake references.
- The user can access the referenced ticket.

## Observable outcomes

- The draft spec starts with ticket-derived context for the user to confirm or edit.
- The draft spec keeps a link back to the originating ticket.
- Tanren remains the system of record for the spec after creation.

## Out of scope

- Two-way synchronization with the external ticket.
- Creating outbound issues.

## Related

- B-0018
- B-0052
- B-0057
