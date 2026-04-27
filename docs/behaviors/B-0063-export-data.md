---
id: B-0063
title: Export account or project data for backup or migration
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` with the required permission can export their
account's data or a project's data, so that they can back it up, migrate
to another Tanren installation, or retain a copy outside the system.

## Preconditions

- For account-scope export: the user is exporting their own account data.
- For project-scope export: the user has permission to export the project.
  In organizational contexts this permission may be restricted by
  organization policy.

## Observable outcomes

- The user can initiate an export at the account level or the project
  level and choose what to include (specs, loops and their history,
  milestones, initiatives, configuration, external references).
- The export is produced as a downloadable artifact the user can save
  outside Tanren.
- The export does not include user-tier credentials belonging to other
  users, even when exporting a project they are a member of.
- The user can see the status of an in-progress export and cancel it.
- A completed export is self-contained enough to be restored per B-0064
  or inspected manually.

## Out of scope

- Continuous synchronization to an external store (this behavior covers
  point-in-time exports, not live backups).
- Exporting data the user does not have visibility of.

## Related

- B-0049
- B-0050
- B-0064
