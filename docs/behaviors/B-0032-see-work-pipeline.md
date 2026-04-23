---
id: B-0032
title: See the work pipeline — velocity and throughput
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can see the flow of specs through the system —
how many specs are in each lifecycle state right now (throughput) and how
many are being completed over time (velocity) — so that they can tell
whether work is moving and at what rate.

## Preconditions

- Has visibility scope over the projects, milestones, initiatives, or teams
  being viewed.

## Observable outcomes

- The user can see a current snapshot of how many specs are in each spec
  lifecycle state (see concepts.md).
- The user can see a completion rate (specs reaching done) over the selected
  time window.
- Both views respect the scope selected via B-0037 (project, milestone,
  initiative, or team) and the time window selected via B-0035.
- The view is legible on a phone.

## Out of scope

- Defining "ideal" velocity targets — this behavior reports, it does not
  judge.
- Forecasting completion dates.

## Related

- B-0033
- B-0034
- B-0035
- B-0037
