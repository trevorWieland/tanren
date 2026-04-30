---
schema: tanren.behavior.v0
id: B-0053
title: See external tickets that are not yet shaped into specs
area: external-tracker
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can see tickets in the project's connected
external tracker that do not yet have a corresponding Tanren spec, so that
nothing sits waiting to be shaped because no one noticed it.

## Preconditions

- The project has a connected external tracker (B-0052).
- The user has visibility of the project.

## Observable outcomes

- The user can see a list of tickets in the connected tracker that are
  candidates for shaping but do not yet have a corresponding spec in the
  project.
- From the list the user can start a shape-spec process (B-0018) on any
  listed ticket.
- The view is accessible on a phone; on any interface the user can tell at
  a glance whether there are unshaped tickets waiting.

## Out of scope

- Deciding for the user which tickets should be shaped — the list is
  informational.
- Hiding tickets based on priority, author, or age.

## Related

- B-0018
- B-0052
