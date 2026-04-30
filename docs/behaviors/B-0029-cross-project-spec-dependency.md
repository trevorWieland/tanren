---
schema: tanren.behavior.v0
id: B-0029
title: Honor cross-project spec dependencies
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can rely on Tanren to honor a spec dependency that
points to a spec in another project, so that work that crosses project
boundaries is coordinated the same way work within a single project is.

## Preconditions

- The depending spec is in a project connected to an account.
- The depending spec has a declared dependency on a spec or ticket owned
  by another project. Declared dependencies may originate from an
  external ticket during spec creation (B-0018) or be recorded directly
  on the spec; either source is supported.

## Observable outcomes

- Dependencies that point to specs in other connected projects under the same
  account appear in the depending spec alongside same-project dependencies.
- B-0017 blocks loop starts for cross-project dependencies the same way it
  does for same-project dependencies.
- If a dependency points to a project the user's active account does not have
  connected, the dependency is shown as unresolved, and the user is told what
  is missing.
- The user can navigate from a cross-project dependency to the dependency's
  own project (subject to their visibility there).

## Out of scope

- Cross-account dependencies — dependencies may only resolve within a single
  account.
- Automatically connecting a missing project to resolve a dangling
  dependency.

## Related

- B-0017
- B-0018
- B-0025
