---
id: B-0064
title: Restore account or project data from a backup
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` with the required permission can restore an
account's or a project's data from a previously created export (B-0063),
so that recovery from accidental loss or migration from another Tanren
installation is possible.

## Preconditions

- The user has an export artifact produced by B-0063.
- For account-scope restore: the user is restoring into their own account.
- For project-scope restore: the user has permission to restore the
  project. In organizational contexts this permission may be restricted by
  organization policy.
- The target location for the restore is in a state where a restore is
  valid (for example, an empty project, or a project explicitly being
  rolled back).

## Observable outcomes

- The user can initiate a restore from an export artifact and choose
  whether to restore everything or a subset.
- The user sees a preview of what will be created, changed, or replaced
  before confirming.
- A restore is attributed in the relevant permission and configuration
  change histories (B-0042) so it is traceable.
- If the restore cannot proceed safely — for example because it would
  conflict with existing active loops — the user is told why and nothing
  is changed.

## Out of scope

- Cross-account restores that merge data between accounts.
- Partial or selective rollback based on time range rather than artifact.

## Related

- B-0042
- B-0063
