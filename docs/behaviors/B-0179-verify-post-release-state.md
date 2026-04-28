---
id: B-0179
title: Verify post-release state
area: release-learning
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
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
- Verification evidence links back to the release, roadmap, and specs where applicable.

## Out of scope

- Requiring every project to have the same deployment model.
- Inferring external production health without configured evidence.

## Related

- B-0034
- B-0143
- B-0181
