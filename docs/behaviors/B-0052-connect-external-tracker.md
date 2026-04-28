---
id: B-0052
title: Connect external tracker capabilities for a project
area: external-tracker
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with the required permission can connect
external tracker capabilities for a project so tracker tickets can be referenced
for intake and Tanren-generated follow-up work can be filed outward when
enabled.

## Preconditions

- The user has permission to change project-tier configuration for the
  project (see B-0049).
- The user has authorization sufficient for the selected tracker capability:
  reading references, filing outbound issues, or both.

## Observable outcomes

- The user can select a supported tracker and enable intake references,
  outbound issue filing, or both.
- Tanren can reference accessible tracker tickets while creating specs when
  intake is enabled.
- Specs created from tracker tickets keep a link back to the originating
  ticket.
- Generated outbound issues use the configured destination when outbound filing
  is enabled.
- The enabled tracker capabilities are visible in project configuration.

## Out of scope

- Building or configuring the external tracker itself.
- Disconnecting or replacing a tracker connection.
- Two-way state sync between a Tanren spec and an external ticket — the
  relationship remains one-way per concepts.md.

## Related

- B-0018
- B-0049
- B-0075
- B-0091
