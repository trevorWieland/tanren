---
id: B-0006
title: Trigger a walk to review a completed loop
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can trigger a walk for a spec whose implementation
loop has finished, so that the implemented behaviors are demoed and reviewed
before the spec is considered done.

A walk reviews the spec through a demonstration of the behaviors it was
supposed to deliver. It is not a code review. For a `team-dev`, the walk is
shared with one or more other developers. For a `solo-dev` with no peers
available, the walk is a self-review of the same demoed behaviors.

## Preconditions

- The spec's implementation loop has completed.
- All audits have passed and there are no outstanding blockers.

## Observable outcomes

- Triggering a walk transitions the spec into a walk state visible to anyone
  with visibility of the spec.
- The walk is the explicit gate between "implementation complete" and
  "spec done" — a completed loop does not auto-close the spec.
- The user can see whether a walk is pending, in progress, or finished.
- A `solo-dev` can proceed through a walk without another reviewer.

## Out of scope

- The mechanics of the walk itself (how demos are presented, what evidence is
  surfaced, how sign-off is captured).
- The decision of when and how the spec finally closes after the walk.
- Code review of the implementation — Tanren does not require developers to
  review source code as part of a walk.

## Related

- B-0001
- B-0003
