---
schema: tanren.behavior.v0
id: B-0181
title: Ingest post-ship feedback
area: release-learning
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can ingest post-ship bugs, support feedback, metrics, or complaints so shipped outcomes feed future planning.

## Preconditions

- Shipped work or a release exists.
- The user has permission to add product feedback or outcome source signals.

## Observable outcomes

- Feedback is linked to the shipped work or product area it concerns.
- Tanren distinguishes bugs, requests, complaints, metrics, and observations when source signals support it.
- The user can route feedback to candidate work, roadmap revision, or no action with rationale.

## Out of scope

- Automatically collecting private customer data without configuration.
- Treating every complaint as accepted roadmap work.

## Related

- B-0094
- B-0179
- B-0182
