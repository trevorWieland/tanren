---
schema: tanren.behavior.v0
id: B-0136
title: Complete first-run setup
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can complete first-run setup so Tanren is ready to manage real project work.

## Preconditions

- The user has access to a Tanren installation or local Tanren binary.
- The user can create or select an account.

## Observable outcomes

- Tanren shows which setup decisions remain before project work can begin.
- Completed setup choices are recorded and visible for review.
- The user can proceed to create or connect a first project.

## Out of scope

- Granting project or organization permissions.
- Creating a product roadmap during setup.

## Related

- B-0025
- B-0026
- B-0137
