---
id: B-0124
title: Detect intent conflicts after related work lands
area: review-merge
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or Tanren worker can detect when related landed work changes the intent assumptions of in-flight work so semantic conflicts do not pass as clean merges.

## Preconditions

- Related work has landed while another spec remains in flight or review.
- Tanren has enough spec context to compare relevant intent.

## Observable outcomes

- The affected work records an intent-conflict concern.
- The user can see why the concern was raised.
- Tanren routes remediation through follow-up work or investigation.

## Out of scope

- Perfect semantic conflict detection.
- Blocking unrelated work with vague suspicion.

## Related

- B-0029
- B-0113
- B-0123
