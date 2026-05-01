---
schema: tanren.behavior.v0
id: B-0091
title: Disconnect or replace an external tracker
area: external-tracker
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can disconnect or replace a project external tracker connection so tracker usage can change without losing Tanren state.

## Preconditions

- The project has an external tracker connection.
- The user has permission to manage external tracker configuration.

## Observable outcomes

- The user can disconnect the existing tracker connection.
- The user can replace the tracker connection with another supported destination.
- Existing specs and outbound issue records keep historical links.

## Out of scope

- Deleting issues from the external tracker.
- Migrating external tracker content.

## Related

- B-0052
