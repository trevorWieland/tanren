---
schema: tanren.behavior.v0
id: B-0026
title: Create a new project from scratch
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can create a new project from scratch, so that a
brand new repository comes into being at the same time the project is
registered with Tanren, without requiring the user to set up the repository
externally first.

## Preconditions

- The user is signed into an account under which the project will live.
- The user can designate where the underlying repository should be created
  (e.g. on a git host they have access to, or locally).

## Observable outcomes

- A new repository is created and registered as a project in the user's
  account in a single action.
- The new project is immediately selectable as the active project.
- The new project starts empty — no specs, no milestones, no initiatives.

## Out of scope

- Importing scaffolds, templates, or starter code into the new repository.
- Copying configuration from an existing project.

## Related

- B-0025
- B-0027
