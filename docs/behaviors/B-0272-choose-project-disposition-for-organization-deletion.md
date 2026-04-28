---
id: B-0272
title: Choose project disposition when deleting an organization
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user deleting an organization can choose what happens to each organization
project so deletion does not silently orphan or remove project work.

## Preconditions

- The user has permission to delete the organization.
- The organization owns one or more projects.

## Observable outcomes

- Before deletion, each organization-owned project is listed with active work,
  dependencies, and export status where visible.
- The user chooses whether each project is detached to an eligible account or
  deleted with the organization.
- Destructive project disposition requires explicit confirmation.
- The chosen disposition is recorded before organization deletion completes.

## Out of scope

- Undeleting a deleted organization.
- Deleting an underlying source repository unless a separate provider action is
  configured and approved.
- Moving a project to an account the user cannot administer.

## Related

- B-0030
- B-0063
- B-0067
