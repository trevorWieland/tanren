---
schema: tanren.behavior.v0
id: B-0008
title: Inspect detailed live activity of a running loop
area: implementation-loop
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `observer` can drill into a running implementation
loop to see what it is currently doing in detail, beyond the summary state, so
that they can understand progress or diagnose unusual slowness.

## Preconditions

- Has visibility scope over the loop's spec.
- The loop is running or paused (not yet archived).

## Observable outcomes

- The user can view live activity beyond the high-level stage summary on
  B-0003.
- Activity updates in near-real-time as the loop progresses.
- Detailed activity is accessible across supported interfaces; on a phone the
  presentation may be abridged but the user can still reach it.

## Out of scope

- Editing or intercepting the activity mid-step — pausing (B-0007) and
  answering surfaced questions (B-0005) are the sanctioned intervention points.
- Archival of detailed activity traces for loops that have already completed.

## Related

- B-0003
- B-0007
