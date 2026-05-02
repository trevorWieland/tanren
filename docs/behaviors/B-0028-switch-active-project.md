---
schema: tanren.behavior.v0
id: B-0028
title: Switch the active project within an account
area: project-setup
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `observer` can switch between projects in their
active account quickly, so that they can move their attention from one
project to another without losing context or re-authenticating.

## Preconditions

- The user is signed into an account with more than one project.

## Observable outcomes

- The user can select any project in the account as the active project from
  within the same view — no sign-out or reconfiguration required.
- After switching, views that are scoped to the active project (spec lists,
  loops, milestones) update to the newly selected project.
- Switching is equally easy on a phone as on a laptop.
- The previously active project's state is not disturbed by the switch;
  returning to it later resumes with the same view.

## Out of scope

- Switching accounts (covered in the configuration and credentials area).
- Pinning or reordering projects in the account.

## Related

- B-0027
