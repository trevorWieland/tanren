---
id: B-0003
title: See the current state of an implementation loop
personas: [solo-dev, team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev`, `team-dev`, or `observer` can at any time see the current state
of any implementation loop they have visibility of, so that they can tell
whether it is progressing, paused, errored, or waiting for human feedback.

## Preconditions

- Has visibility scope over the loop's spec.

## Observable outcomes

- The user can see the loop's current stage.
- The user can see whether the loop is running, paused, errored, or waiting
  on a question.
- A running loop that needs no attention is clearly distinguishable from one
  that does.
- Loop state is visible from any supported interface, including on a phone.

## Out of scope

- Step-by-step real-time traces of agent activity.
- Historical timelines of loops that have already completed.

## Related

- B-0004
- B-0005
