---
id: B-0122
title: Clean up completed execution resources
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

A user or Tanren worker can clean up resources for completed work so finished execution does not leave unnecessary worktrees, leases, or runtime targets active.

## Preconditions

- Work has completed, failed terminally, or been cancelled.
- Cleanup is allowed by policy and retention settings.

## Observable outcomes

- Runtime resources are released or marked retained with a reason.
- Evidence needed for audit and recovery remains available.
- Users can see cleanup status.

## Out of scope

- Deleting required evidence.
- Cleaning unrelated work.

## Related

- B-0058
- B-0105
- B-0121
