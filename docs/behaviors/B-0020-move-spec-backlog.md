---
id: B-0020
title: Move a spec to or from the backlog
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can move a shaped spec to the backlog when it is
not yet prioritized, and can later move it back to ready when they want it
picked up, so that a queue of not-yet-prioritized work is kept distinct from
specs ready to run.

## Preconditions

- The spec exists and has been shaped (B-0018).
- The user has permission to change the spec's lifecycle state.

## Observable outcomes

- The spec's state is visible as backlog via B-0021, distinct from draft and
  ready.
- A backlog spec is not eligible to be picked up by automatic start (B-0002)
  and cannot have a loop started on it manually until moved to ready.
- The user can move a backlog spec to ready at any time.

## Out of scope

- Ordering specs within the backlog or relative to each other — this
  behavior only moves a spec in or out of the backlog state.
- Automatic promotion from backlog based on rules or dates.

## Related

- B-0002
- B-0019
- B-0021
