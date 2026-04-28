---
id: B-0093
title: Turn roadmap items into specs
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can turn a roadmap item into one or more specs so planned product direction becomes executable work.

## Preconditions

- A roadmap item exists.
- The user has permission to create specs.

## Observable outcomes

- The generated specs retain a link to the roadmap item.
- The user can review and adjust each spec before it is accepted into the project.
- Dependencies between generated specs can be recorded.

## Out of scope

- Starting implementation immediately.
- Guaranteeing that every roadmap item maps to exactly one spec.

## Related

- B-0018
- B-0077
- B-0092
