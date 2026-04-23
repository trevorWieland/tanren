---
id: B-0018
title: Shape a spec from an external ticket
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can shape a new Tanren spec from an external
ticket (Linear, Jira, GitHub Issues, etc.), so that a high-level problem
description from the business, a customer, or an automated bug report becomes
a Tanren-owned unit of work ready for an implementation loop.

## Preconditions

- An active project is selected.
- The user references an external ticket that describes a problem and, where
  appropriate, expected behaviors or acceptance criteria.

## Observable outcomes

- Tanren walks the user through an interactive shaping process that produces
  a new spec in the project.
- The new spec captures the problem description and acceptance criteria from
  the ticket, without adding implementation detail.
- Any dependencies declared on the external ticket are surfaced during
  shaping, confirmed with the user, and recorded on the new spec so that
  B-0017 can honor them.
- The newly shaped spec is visible in the project and carries a link back to
  the originating external ticket.

## Out of scope

- Authoring a spec without a ticket — Tanren assumes a ticket as the
  problem-statement origin.
- Round-trip sync between the Tanren spec and the external ticket; the ticket
  remains the originator, the Tanren spec becomes authoritative for
  implementation.
- Defining implementation approach or technical design — that is the job of
  the implementation loop, not the spec.

## Related

- B-0017
- B-0019
- B-0021
