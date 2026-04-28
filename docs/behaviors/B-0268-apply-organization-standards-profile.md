---
id: B-0268
title: Apply organization standards profiles to projects
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with standards policy permission can apply organization standards
profiles to projects so required or recommended standards reach project work
without silently overwriting project-owned standards.

## Preconditions

- An organization standards profile exists.
- The user has permission to manage standards policy for the affected project or
  organization.

## Observable outcomes

- The project can see which organization standards profiles are required,
  recommended, or inherited.
- Required profile rules influence subsequent Tanren guidance and policy checks.
- Conflicts between project standards and organization profiles are visible
  before work starts.
- Applying or removing a profile is attributed and visible in change history.

## Out of scope

- Authoring the organization profile itself.
- Automatically deleting project-specific standards.
- Applying standards across organizations.

## Related

- B-0049
- B-0084
- B-0152
- B-0267
