---
id: B-0001
title: Start an implementation loop manually on a spec
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: implemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can start an implementation loop on a selected spec
so that Tanren begins work on that spec.

## Preconditions

- An active project is selected.
- The spec exists in the project.
- A `team-builder` starting a loop on a spec they do not own has permission to do
  so.

## Observable outcomes

- The spec enters a running state that the user can see.
- The user can immediately tell which stage of the loop is active.
- Notifications and audit records for that loop are associated with the user
  who started it.

## Out of scope

- The mechanics of how the loop advances between stages.
- Choosing which harness or execution environment the loop runs in.

## Related

- B-0002
- B-0003
- B-0004
- B-0005
