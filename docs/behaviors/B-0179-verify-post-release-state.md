---
schema: tanren.behavior.v0
id: B-0179
title: Verify post-release state
area: release-learning
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can verify post-release state so Tanren can show whether shipped work appears healthy after release.

## Preconditions

- A release or deployed change exists.
- The user has visibility into the release and configured post-release signals.

## Observable outcomes

- Tanren shows post-release checks, signals, or observations tied to the shipped work.
- Failures or missing signals are visible as follow-up candidates.
- Verification source references back to the release, roadmap, and specs where applicable.

## Out of scope

- Requiring every project to have the same deployment model.
- Inferring external production health without configured source signals.

## Related

- B-0034
- B-0143
- B-0181
