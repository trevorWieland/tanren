---
id: B-0021
title: See a spec's current lifecycle state
personas: [solo-dev, team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev`, `team-dev`, or `observer` can at any time see a spec's current
lifecycle state so that they can tell where it is in the authoring and
delivery pipeline without having to open the spec's loop.

## Preconditions

- Has visibility scope over the spec.

## Observable outcomes

- The user can see which state the spec is in: draft, ready, backlog, blocked,
  in a loop, awaiting walk, done, or archived.
- The user can see the problem description, acceptance criteria, and any
  recorded dependencies carried by the spec.
- State is visible from any supported interface, including a phone.

## Out of scope

- Live implementation-loop activity — that is covered by B-0003 and B-0008.
- Historical state transitions of the spec itself.

## Related

- B-0003
- B-0008
- B-0018
