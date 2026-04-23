---
id: B-0049
title: Manage project-tier configuration
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` with the required permission can manage
configuration specific to a project — such as gate commands that loops
must satisfy, standard folder conventions, and project-scoped secrets —
so that the project's working rules are shared consistently with everyone
who has access to it.

## Preconditions

- The user has permission to change project configuration for the active
  project. In organizational contexts this permission may be restricted by
  organization policy.

## Observable outcomes

- The user can view and edit project-tier configuration values, including
  gate commands, folder conventions, and project-level secrets.
- Project-tier values are visible to everyone with access to the project
  and apply to every loop and action taken within the project.
- Changes take effect for subsequent work; loops already in flight when a
  change is made continue under the settings they started with.
- Every change is attributed and visible in the project's change history
  (B-0042).

## Out of scope

- Per-user overrides of project configuration (settings live at one tier,
  not mixed).
- Automatic migration of configuration between projects.

## Related

- B-0048
- B-0050
- B-0051
