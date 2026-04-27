---
id: B-0030
title: Disconnect a project from Tanren
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can disconnect a project from their account, so
that it no longer appears in their views, without affecting the underlying
repository.

## Preconditions

- The project has no active implementation loops.
- The user has permission to disconnect the project.

## Observable outcomes

- After disconnection, the project stops appearing in the account's project
  lists and views.
- The underlying repository is not deleted or modified by disconnection.
- Cross-project dependencies from other projects that pointed to specs in the
  disconnected project are shown as unresolved (see B-0029).
- A disconnected project can be reconnected later via B-0025, restoring
  access to its specs.

## Out of scope

- Deleting the underlying repository.
- Archiving or exporting the project's Tanren history.

## Related

- B-0025
- B-0029
