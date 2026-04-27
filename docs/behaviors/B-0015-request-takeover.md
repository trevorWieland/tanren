---
id: B-0015
title: Request a takeover from the current owner
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` who lacks ad-hoc takeover permission can request takeover of a
teammate's loop from the current owner, so that ownership can transfer with
explicit consent.

## Preconditions

- An active loop exists and is owned by another `team-dev`.
- The project's coordination rules (B-0012) require consent for takeover, or
  the user otherwise lacks permission to take over ad-hoc.

## Observable outcomes

- The user can send a takeover request to the current owner.
- The current owner is notified of the request and can approve or decline it.
- If approved, ownership transfers per B-0011 and the requester is notified.
- If declined or left unanswered, the loop remains with the original owner
  and the requester is notified of the outcome.
- The request and its resolution appear in the loop's action history
  (B-0014).

## Out of scope

- Forcing takeover against the owner's will — that path is governed by
  pre-granted permissions and ad-hoc rules (B-0012), not by this behavior.
- Multi-approver workflows (e.g. manager must also approve).

## Related

- B-0011
- B-0012
- B-0014
