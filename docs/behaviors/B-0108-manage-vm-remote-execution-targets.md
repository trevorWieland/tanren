---
id: B-0108
title: Manage VM or remote execution targets
area: runtime-substrate
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can manage VM or remote execution targets so Tanren can run work away from the local machine when appropriate.

## Preconditions

- The user has permission to manage execution targets.
- The installation supports remote or VM-backed execution.

## Observable outcomes

- The user can add, inspect, update, and remove execution targets.
- Targets show whether they are available for work.
- Policy determines which projects or organizations may use each target.

## Out of scope

- Cloud-provider-specific provisioning details.
- Bypassing placement policy.

## Related

- B-0081
- B-0102
- B-0109
