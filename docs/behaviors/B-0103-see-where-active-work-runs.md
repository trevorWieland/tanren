---
schema: tanren.behavior.v0
id: B-0103
title: See where active work is running
area: runtime-substrate
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see where active Tanren work is running so execution is inspectable without knowing runtime internals.

## Preconditions

- The user has visibility of the active work.

## Observable outcomes

- The active work view identifies the execution target class and current state.
- The user can distinguish local, hosted, remote, or VM-backed execution at a high level.
- The view avoids exposing host secrets or low-level infrastructure identifiers unnecessarily.

## Out of scope

- Direct shell access to the environment.
- Provider-specific infrastructure debugging.

## Related

- B-0102
- B-0104
- B-0130
