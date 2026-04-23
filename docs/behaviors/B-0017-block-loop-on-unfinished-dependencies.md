---
id: B-0017
title: Block starting a loop on a spec with unfinished dependencies
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A user who attempts to start a loop on a spec whose declared dependencies are
not yet complete is blocked from starting and is shown what is blocking them,
so that foundation work is not raced ahead of.

## Preconditions

- The user is attempting to start an implementation loop via B-0001 or via
  automatic triggering via B-0002.
- The spec has one or more declared dependencies that are not finished.

## Observable outcomes

- The start attempt is blocked; no loop begins.
- The user is shown which specs (or external issues) are blocking the start,
  including their current state where visible.
- Blocking specs may live in another project connected to the same account;
  cross-project dependencies are honored the same way as same-project ones.
- From that view the user can navigate to a blocking spec to check on it or
  act on it (subject to their own permissions).
- Once all dependencies finish, a new start attempt for the spec succeeds
  normally — including automatic starts via B-0002.

## Out of scope

- Declaring dependencies on a spec (covered by spec authoring behaviors).
- Soft warnings for adjacent but undeclared work — this behavior only acts on
  declared dependencies.
- Overriding the block — explicitly not supported at this level.

## Related

- B-0001
- B-0002
- B-0013
