---
id: B-0050
title: Manage shared configuration defaults across projects
area: configuration
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `operator` with the required permission can
manage shared defaults that apply across multiple projects, so repeated
deployment, runtime, and development settings do not have to be configured one
project at a time.

## Preconditions

- The user has permission to manage shared defaults for the active account or
  organization.
- More than one project can use the shared defaults.

## Observable outcomes

- The user can view and edit shared default values such as deployment posture,
  runtime preferences, linked provider defaults, and project-creation defaults.
- Shared defaults are visible to users with access to the affected scope and can
  apply across every project in that account or organization.
- A one-person multi-project setup can use shared defaults without adopting
  team-specific concepts such as member roles or team tracking.
- Changes take effect for subsequent work; loops already in flight when a
  change is made continue under the settings they started with.
- Every change is attributed and visible in the relevant change history
  (B-0042).

## Out of scope

- Cross-organization defaults.
- Shared secret storage.
- Role, permission, or team-member tracking.
- Per-project overrides when policy forbids them.

## Related

- B-0040
- B-0048
- B-0049
- B-0051
- B-0269
