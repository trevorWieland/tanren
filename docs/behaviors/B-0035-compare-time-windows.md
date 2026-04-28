---
id: B-0035
title: Compare metrics across time windows
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can pick a time window — or compare two time windows — when viewing
velocity, throughput, quality, and health signals, so that they can see
historical trends rather than only the current moment.

## Preconditions

- Has visibility scope over the work being viewed.

## Observable outcomes

- The user can select a time window (e.g. last 7 days, last quarter, custom
  range) for any observation view.
- Where relevant, the user can select two windows and see a side-by-side or
  delta comparison between them (e.g. this quarter vs last quarter).
- A default window is applied if the user has not chosen one, so the user
  does not need to configure anything to get value.
- The selected window is honored by B-0032, B-0033, and B-0034.

## Out of scope

- Predictive analytics or forecasting.
- Saving named time windows for reuse.

## Related

- B-0032
- B-0033
- B-0034
