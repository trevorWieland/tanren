---
id: B-0007
title: Manually pause and resume an active loop
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can manually pause an active implementation loop
and later resume it, so that they can halt Tanren's work on a spec without
abandoning the progress it has made.

## Preconditions

- The loop is in a running state.
- The user has permission to act on the loop.

## Observable outcomes

- The user can pause a running loop at any time without losing progress.
- A manually-paused loop reports a paused state visible to anyone with
  visibility, and is distinguishable from a loop paused by a blocker.
- The user can resume a manually-paused loop, and it continues from where it
  paused.

## Out of scope

- Cancelling or aborting a loop (discarding progress) — covered by
  B-0058.
- Rolling back partial progress of a paused loop.

## Related

- B-0003
- B-0005
- B-0058
