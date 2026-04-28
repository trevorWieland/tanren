---
id: B-0018
title: Create a draft spec manually
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can create a draft spec directly in a project so
that a problem becomes a Tanren-owned unit of work before it is prioritized or
implemented.

## Preconditions

- An active project is selected.

## Observable outcomes

- The user can create a new draft spec without an external tracker.
- The draft spec captures a user-authored problem description.
- The draft spec is visible in the project as draft work.
- Creation does not require implementation approach or technical design.

## Out of scope

- Prefilling a spec from an external ticket.
- Defining acceptance criteria.
- Declaring dependencies.
- Marking the spec ready to run.

## Related

- B-0017
- B-0019
- B-0021
- B-0075
- B-0076
- B-0077
- B-0078
