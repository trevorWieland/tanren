---
id: B-0025
title: Connect Tanren to an existing repository
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can connect Tanren to a repository that already
exists, so that the repository becomes a project the user can shape specs
against and run implementation loops on.

## Preconditions

- The user is signed into an account under which the project will live.
- The user has access to the repository being connected.

## Observable outcomes

- The repository appears as a project in the user's account and is available
  to be selected as the active project.
- The project is scoped to exactly one repository — a polyrepo setup requires
  connecting each repository as its own project.
- The user can disconnect the project later without deleting or modifying the
  underlying repository (see B-0030).

## Out of scope

- Importing an existing repository's history as Tanren activity. Connection
  is forward-looking — prior commits are not reinterpreted as Tanren work.
- Multi-repo or monorepo-as-one-project setups.

## Related

- B-0026
- B-0027
- B-0030
