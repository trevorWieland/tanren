---
id: B-0014
title: See the history of human actions on a loop
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can see a chronological record of the human actions
taken on a loop — starts, pauses, resumes, blocker responses, assists,
transfers — so that they can tell who did what and when, especially on loops
shared across teammates.

## Preconditions

- Has visibility scope over the loop.

## Observable outcomes

- The user can see an ordered list of human actions on the loop, each
  attributed to the user who performed it and stamped with when it happened.
- The history is available for both active loops and loops that have finished.
- Entries that reflect assisting actions (B-0010) and ownership transfers
  (B-0011) are clearly marked so it is obvious the actor was not the owner.

## Out of scope

- A second-by-second trace of autonomous agent activity (covered by B-0008 for
  live loops).
- Editing or redacting history entries.

## Related

- B-0008
- B-0010
- B-0011
