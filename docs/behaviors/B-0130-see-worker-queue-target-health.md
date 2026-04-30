---
schema: tanren.behavior.v0
id: B-0130
title: See daemon, worker, queue, and execution-target health
area: operations
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see Tanren operational health so stuck queues, unhealthy workers, or unavailable execution targets are visible before they block delivery silently.

## Preconditions

- The user has visibility of the project, organization, or installation health scope.

## Observable outcomes

- The user can see worker, queue, daemon, and execution target status at a high level.
- Health signals distinguish normal, degraded, unavailable, and draining states.
- The view avoids leaking unauthorized infrastructure detail.

## Out of scope

- Replacing provider observability tools.
- Showing secret values or host-level credentials.

## Related

- B-0034
- B-0103
- B-0109
