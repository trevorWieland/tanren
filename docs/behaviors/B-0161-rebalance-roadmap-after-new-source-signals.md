---
schema: tanren.behavior.v0
id: B-0161
title: Rebalance the roadmap after new source signals
area: prioritization
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can rebalance the roadmap after new source signals so product planning adapts without losing the prior rationale.

## Preconditions

- A roadmap exists.
- New feedback, audit findings, product decisions, or post-ship outcomes have been recorded.

## Observable outcomes

- Tanren identifies roadmap items affected by the new source signals.
- Proposed changes show what would move, split, defer, or become newly important.
- Accepted roadmap changes preserve links to the source signals that motivated them.

## Out of scope

- Silently rewriting roadmap history.
- Automatically executing newly prioritized work without execution policy.

## Related

- B-0092
- B-0113
- B-0182
