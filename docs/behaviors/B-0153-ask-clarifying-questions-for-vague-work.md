---
schema: tanren.behavior.v0
id: B-0153
title: Ask clarifying questions for vague work
area: spec-quality
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can receive clarifying questions for vague work so specs become specific enough to shape safely.

## Preconditions

- Candidate work, a draft spec, or roadmap item exists.
- The user has permission to edit the work item.

## Observable outcomes

- Tanren identifies the unclear parts of the work.
- Questions are tied to user-visible product, behavior, risk, or acceptance concerns.
- User answers become traceable shaping context.

## Out of scope

- Asking questions about internal implementation choices too early.
- Marking work ready while essential ambiguity remains unresolved.

## Related

- B-0018
- B-0078
- B-0157
