---
schema: tanren.behavior.v0
id: B-0022
title: Archive a spec without implementation
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can archive a spec that will not be implemented
(obsolete, rejected, duplicate, deprioritized) so that it stops appearing in
active views without being deleted or losing the record of why it existed.

## Preconditions

- The spec exists and does not have an active implementation loop.
- The user has permission to change the spec's lifecycle state.

## Observable outcomes

- The archived spec no longer appears in default views of active or ready
  work.
- The spec is still reachable through explicit searches or history views and
  retains its problem description, acceptance criteria, and link back to the
  originating ticket.
- The user can unarchive a spec to return it to a usable state (draft, ready,
  or backlog).

## Out of scope

- Deleting a spec permanently — archive is non-destructive.
- Automatic archival based on age or inactivity.

## Related

- B-0018
- B-0021
