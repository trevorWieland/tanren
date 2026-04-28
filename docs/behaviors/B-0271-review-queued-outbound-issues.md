---
id: B-0271
title: Review queued outbound issues
area: external-tracker
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with outbound issue review permission can review queued outbound issues
so Tanren-generated follow-up work is approved, revised, or discarded before it
is filed externally.

## Preconditions

- The project has outbound issue review mode enabled.
- One or more outbound issues are queued for review.
- The user has permission to review outbound issues.

## Observable outcomes

- The user can inspect each queued issue with its originating spec, loop,
  finding, or review-feedback context.
- The user can edit the title or description before approval.
- The user can approve the issue for external filing or discard it with a
  rationale.
- The decision is attributed and visible from Tanren's outbound issue history.

## Out of scope

- Multi-person approval workflows for a single outbound issue.
- Deleting issues already filed in the external tracker.
- Changing external tracker state after filing except through configured
  outbound behavior.

## Related

- B-0054
- B-0055
- B-0119
