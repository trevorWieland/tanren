---
id: B-0018
title: Create a spec, optionally from an external ticket
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can create a new spec in a project. When the
project has a connected external tracker, the creation flow can pull
details from an external ticket to pre-fill the new spec, so that a
problem description from the business, a customer, or an automated bug
report becomes a Tanren-owned unit of work without re-typing.

## Preconditions

- An active project is selected.
- For ticket-based creation: the project has a connected external tracker
  (B-0052) and the user can reference an accessible ticket within it.

## Observable outcomes

- Tanren walks the user through an interactive creation process that
  produces a new spec in the project.
- When an external ticket is referenced, the spec is pre-populated with
  the ticket's problem description, acceptance criteria, and any declared
  dependencies; the user confirms or adjusts each item before the spec is
  saved.
- When no ticket is referenced, the user authors the problem description
  and acceptance criteria directly; creation does not require a tracker.
- The spec captures the problem description and acceptance criteria
  without adding implementation detail.
- Declared dependencies, whether pulled from a ticket or entered by the
  user, are recorded on the new spec so B-0017 can honor them.
- A spec created from a ticket carries a link back to the originating
  external ticket; a spec created directly does not.

## Out of scope

- Round-trip sync between a Tanren spec and an external ticket.
- Defining implementation approach or technical design — that is the job
  of the implementation loop.

## Related

- B-0017
- B-0019
- B-0021
- B-0052
