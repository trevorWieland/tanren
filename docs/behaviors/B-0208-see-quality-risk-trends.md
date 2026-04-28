---
id: B-0208
title: See quality and risk trends over time
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see quality and risk trends over time so project health can be judged beyond the latest result.

## Preconditions

- The selected scope has quality, review, finding, blocker, or outcome history.
- The user has visibility into the trend inputs.

## Observable outcomes

- Tanren shows whether quality and risk signals are improving, degrading, or stable.
- Trend summaries link to representative evidence and time windows.
- Missing or low-volume data is called out before drawing conclusions.

## Out of scope

- Reducing quality to a single unexplained score.
- Ranking people by quality signals.

## Related

- B-0033
- B-0035
- B-0080
