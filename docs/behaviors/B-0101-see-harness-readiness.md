---
schema: tanren.behavior.v0
id: B-0101
title: See whether a harness is ready to run work
area: runtime-substrate
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see whether a selected harness is ready to run work so failures caused by missing credentials, policy, or setup are understandable before dispatch.

## Preconditions

- The user has visibility of the project or runtime settings.

## Observable outcomes

- The user can see ready, blocked, or unavailable harness status.
- Unavailable status explains whether credentials, policy, or setup is missing.
- The status does not expose credential values.

## Out of scope

- Testing provider internals.
- Bypassing policy blocks.

## Related

- B-0099
- B-0100
