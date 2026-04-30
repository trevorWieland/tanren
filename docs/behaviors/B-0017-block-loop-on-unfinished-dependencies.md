---
schema: tanren.behavior.v0
id: B-0017
title: Block starting a loop when declared dependencies are not usable
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` who attempts to start a loop on a spec whose
declared dependencies are not yet usable can discover which dependencies block
execution and act on them, rather than racing ahead of unavailable foundation
work.

## Preconditions

- The user is attempting to start an implementation loop via B-0001 or via
  automatic triggering via B-0002.
- The spec has one or more declared dependencies that are not usable under the
  project's dependency mode.

## Observable outcomes

- The start attempt is blocked; no loop begins.
- The user is shown which specs (or external issues) are blocking the start,
  including their current state or usable base where visible.
- Blocking specs may live in another project connected to the same account;
  cross-project dependencies are honored the same way as same-project ones.
- A dependency can be usable before it is merged to the primary branch when an
  approved stacked-diff or branch-based dependency flow makes its base available.
- From that view the user can navigate to a blocking spec to check on it or
  act on it (subject to their own permissions).
- Once all dependencies are usable, a new start attempt for the spec succeeds
  normally — including automatic starts via B-0002.

## Out of scope

- Declaring dependencies on a spec (covered by spec authoring behaviors).
- Soft warnings for adjacent but undeclared work — this behavior only acts on
  declared dependencies.
- Overriding the block — explicitly not supported at this level.
- Managing stacked-diff rebases after a dependency lands (covered by B-0266).

## Related

- B-0001
- B-0002
- B-0013
- B-0266
