---
id: B-0050
title: Manage organization-tier configuration
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` who holds the permission to manage organization
configuration can manage settings that apply across every project in an
organization — typically deployment-related settings and shared
infrastructure secrets — so that baseline operational config is maintained
in one place rather than per project.

## Preconditions

- The user has permission to manage organization configuration for the
  active organization.
- The context is organizational; this behavior does not apply to personal
  projects.

## Observable outcomes

- The user can view and edit organization-tier configuration values,
  including deployment-related settings and organization-shared secrets.
- Organization-tier values are visible to every member of the organization
  and apply across every project in the organization.
- Changes take effect for subsequent work; loops already in flight when a
  change is made continue under the settings they started with.
- Every change is attributed and visible in the organization's change
  history (B-0042).

## Out of scope

- Cross-organization defaults.
- Per-project overrides of organization-tier values (projects can layer
  their own settings at the project tier, but cannot override the
  organization's tier).

## Related

- B-0040
- B-0048
- B-0049
- B-0051
