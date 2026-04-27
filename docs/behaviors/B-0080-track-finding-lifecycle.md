---
id: B-0080
title: Track finding lifecycle
personas: [solo-dev, team-dev]
interfaces: [cli, mcp]
contexts: [personal, organizational]
status: accepted
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can see which audit, adherence, demo, and gate
findings are still open so implementation readiness is based on current typed
state, not stale projected artifacts.

## Preconditions

- A spec is being checked by Tanren methodology phases.
- Check phases may raise findings that require remediation before readiness.

## Observable outcomes

- New findings are projected as open until explicitly resolved, reopened,
  deferred, or superseded.
- Open `fix_now` findings block readiness and check completion.
- Remediation tasks preserve typed links to source checks, source findings,
  investigation attempts, and root causes.
- Implementation phases can record evidence but cannot resolve findings.
- Audit artifacts show historical findings while counting only open blockers.

## Out of scope

- Automatically resolving historical findings based on later passing checks.
- Scheduling or prioritizing deferred remediation outside the finding lifecycle.

## Related

- B-0003
- B-0006
- B-0021
- `docs/behaviors/` — **what** users can do (this directory)
