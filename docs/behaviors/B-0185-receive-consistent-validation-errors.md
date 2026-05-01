---
schema: tanren.behavior.v0
id: B-0185
title: Receive consistent validation errors across public interfaces
area: cross-interface
personas: [solo-builder, team-builder, observer, operator, integration-client]
runtime_actors: [agent-worker]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user, integration client, or agent worker can receive consistent validation errors across public interfaces so failures are understandable and automatable.

## Preconditions

- An attempted action fails validation through a public Tanren interface.
- The actor has permission to know that the attempted scope exists.

## Observable outcomes

- Equivalent invalid requests report the same user-visible reason across interfaces.
- Error details identify missing input, policy denial, state conflict, or unavailable capability without leaking secrets.
- Machine-readable interfaces expose consistent, distinguishable error categories so automation can respond reliably to specific failure modes.

## Out of scope

- Making every interface use identical wording.
- Revealing hidden resources through validation details.

## Related

- B-0106
- B-0157
- B-0183
