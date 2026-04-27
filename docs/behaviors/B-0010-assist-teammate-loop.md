---
id: B-0010
title: Temporarily assist a teammate's in-flight loop
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` can act on a teammate's in-flight loop without taking ownership,
so that progress continues while the teammate is unavailable and the work
still belongs to its original owner when they return.

## Preconditions

- The teammate has an active loop the user has visibility of.
- The user has permission to assist that teammate's work.

## Observable outcomes

- The user can answer questions surfaced by the loop on the teammate's behalf
  (see B-0005).
- The user can pause and resume the loop on the teammate's behalf (see B-0007).
- Ownership of the loop does not change; the original owner remains the
  primary recipient of notifications.
- Every assisting action is attributed to the user who performed it, so the
  teammate can see what was done in their absence.

## Out of scope

- Transferring ownership to the assisting user (see B-0011).
- Deciding *who* can assist *whom* — that is governed by B-0012.

## Related

- B-0005
- B-0007
- B-0011
- B-0012
