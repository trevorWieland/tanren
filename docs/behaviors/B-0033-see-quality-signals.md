---
id: B-0033
title: See quality signals across work
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see quality signals about the work Tanren is producing — walk
outcomes, audit flags, blocker frequency — so that they can judge not just how
much is getting done but whether the result is good.

## Preconditions

- Has visibility scope over the work being viewed.

## Observable outcomes

- The user can see the rate at which walks are passing and how often walks
  surface issues that send a spec back for further work.
- The user can see the frequency and type of audit flags raised during
  implementation loops.
- The user can see how often loops are pausing on blockers and what
  categories of blockers are most common.
- Signals respect the scope selected via B-0037 and the time window selected
  via B-0035.

## Out of scope

- Ranking builders or teams by quality — this behavior reports signals, it
  does not rank.
- Customizable quality thresholds or alerts.

## Related

- B-0032
- B-0034
- B-0035
- B-0037
