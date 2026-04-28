---
id: B-0203
title: Manage organization member access
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can manage existing organization members' permissions so organization-level access can change deliberately.

## Preconditions

- The user has permission to manage organization access.
- The target person is already a member of the organization.

## Observable outcomes

- The user can modify an existing member's organization-level permissions.
- The user can grant individual permissions directly or apply a role template that grants its bundled permissions at that moment.
- Changes are attributed and visible in the organization's change history.
- Organization-level membership does not itself grant access to every project.

## Out of scope

- Inviting new members.
- Removing members entirely from the organization.
- Managing project-level access.

## Related

- B-0042
- B-0044
- B-0065
