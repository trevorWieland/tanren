---
id: B-0040
title: See and configure organization-level policy
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` with organization-admin permission can see and configure
policies that apply across every project in the organization, so that
baseline rules are enforced consistently without relying on each project to
configure them independently.

## Preconditions

- The user has organization-admin permission.
- The context is organizational; this behavior does not apply to personal
  projects.

## Observable outcomes

- The user can define policies at the organization level, including:
  - Caps on what permissions individual projects may grant (for example,
    disallowing ad-hoc takeover for every project in the organization).
  - Mandatory restrictions (for example, requiring explicit permission for
    per-developer breakdowns per B-0036).
  - Organization-level roles that are available to every project.
- Organization policies override conflicting project-level configuration.
- The active organization policy is visible to every member of the
  organization, not only admins.

## Out of scope

- Cross-organization policies or meta-organizations.
- Policies that vary dynamically by time, user attribute, or project state.

## Related

- B-0012
- B-0031
- B-0038
