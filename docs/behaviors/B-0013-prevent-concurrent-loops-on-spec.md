---
id: B-0013
title: Prevent concurrent loops on the same spec
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A user who attempts to start a loop on a spec that already has an active loop
is blocked from starting a second loop, and is directed to the existing loop
so they can take appropriate action. Each spec has at most one active loop at
a time.

## Preconditions

- The user is attempting to start an implementation loop via B-0001.
- Another active loop already exists for the same spec.

## Observable outcomes

- The start attempt is blocked; no second loop begins.
- The user is shown the existing loop, including who owns it and its current
  state.
- From that view the user can take the actions available to them: observing
  the loop (B-0003), assisting it (B-0010), or requesting or performing a
  takeover (B-0011) — each subject to their own permissions.
- A spec whose active loop has finished or been closed can have a new loop
  started against it normally.

## Out of scope

- Queueing a start attempt so it runs after the existing loop finishes
  (autostart is covered by B-0002).
- Running multiple exploratory attempts in parallel — explicitly unsupported.

## Related

- B-0001
- B-0003
- B-0010
- B-0011
