---
schema: tanren.behavior.v0
id: B-0122
title: Clean up completed execution resources
area: review-merge
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or Tanren worker can clean up resources for completed work so finished execution does not leave unnecessary workspace clones, leases, or runtime targets active.

## Preconditions

- Work has completed, failed terminally, or been cancelled.
- Cleanup is allowed by policy and retention settings.

## Observable outcomes

- Runtime resources are released or marked retained with a reason.
- Source signals needed for audit and recovery remain available.
- Users can see cleanup status.

## Out of scope

- Deleting required source signals.
- Cleaning unrelated work.

## Related

- B-0058
- B-0105
- B-0121
