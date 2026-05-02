---
schema: tanren.behavior.v0
id: B-0149
title: Discover project standards from repo source signals
area: standards-evolution
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can discover project standards from repository source signals so Tanren guidance reflects the project's actual conventions.

## Preconditions

- An active project is connected to a repository.
- The user has permission to inspect repository source signals.

## Observable outcomes

- Tanren identifies candidate standards with examples from the repository.
- Candidate standards remain separate from accepted standards until reviewed.
- Accepted standards can be used by later planning, shaping, and execution work.

## Out of scope

- Treating every existing pattern as a standard.
- Overwriting user-authored standards without approval.

## Related

- B-0049
- B-0071
- B-0150
