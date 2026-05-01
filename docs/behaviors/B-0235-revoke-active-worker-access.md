---
schema: tanren.behavior.v0
id: B-0235
title: Revoke active worker access
area: runtime-substrate
personas: [operator]
runtime_actors: [agent-worker]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can revoke active worker access so execution access can be stopped during incidents, policy changes, or cancellation.

## Preconditions

- A worker has active temporary access or an active execution lease.
- The user has permission to revoke worker access for the scope.

## Observable outcomes

- Future use of the revoked access fails with a clear Tanren-level reason.
- Affected work and leases show whether they stopped, paused, failed, or need recovery.
- The revocation is attributed and visible in audit history.

## Out of scope

- Destroying source signals needed for investigation.
- Hiding that work was interrupted by access revocation.

## Related

- B-0105
- B-0129
- B-0234
