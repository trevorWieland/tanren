---
id: B-0202
title: Manage project access
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can manage project access so people receive the project visibility and permissions they need.

## Preconditions

- The user has permission to manage the project's access list.
- The project exists in the active account.

## Observable outcomes

- The user can add a person with a chosen visibility and permission set.
- The user can modify or revoke an existing person's project access.
- The user can grant individual permissions directly or apply a role template that grants its bundled permissions at that moment.
- Every change is attributed and visible in the project's change history.

## Out of scope

- Treating roles as authorization identities.
- Managing organization-level membership.
- Bypassing organization policy that constrains project access.

## Related

- B-0031
- B-0038
- B-0042
